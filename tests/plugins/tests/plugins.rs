//! Independent plugin contracts, export archives and failed-operation invariants.
use dioxus_mind_elixir::{
    LayoutDirection, MindMapData, MindMapError, MindMapState, Node, Operation, Placement, Point,
    plugins::{
        self, Capability, Command, Labels, Plugin, PluginRegistry, export,
        gesture::{self, HitRect},
    },
    text,
};
use mind_elixir_plugin_tests::fixture;
use std::io::{Cursor, Read};
type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
#[test]
fn registry_lifecycle_and_isolation() -> Result {
    let mut a = PluginRegistry::default();
    let b = PluginRegistry::default();
    for plugin in Plugin::builtins() {
        a.register(plugin)?;
    }
    assert!(a.provides(Capability::Keyboard));
    assert!(!b.provides(Capability::Keyboard));
    assert!(a.register(Plugin::new("builtin.Keyboard", vec![])).is_err());
    a.set_enabled("builtin.Keyboard", false)?;
    assert!(!a.provides(Capability::Keyboard));
    a.set_enabled("builtin.Keyboard", true)?;
    a.unregister("builtin.Keyboard")?;
    assert!(!a.provides(Capability::Keyboard));
    assert!(a.set_enabled("missing", true).is_err());
    Ok(())
}
#[test]
fn commands_share_atomic_history() -> Result {
    for command in [
        Command::Child,
        Command::SiblingBefore,
        Command::SiblingAfter,
        Command::Parent,
        Command::Duplicate,
        Command::Delete,
        Command::Fold,
        Command::Indent,
    ] {
        let data = fixture();
        let mut state = MindMapState::new(data.clone())?;
        let operation = plugins::command_operation(&data, &["build".into()], command, "新节点")?;
        state.apply(operation)?;
        assert_eq!(state.history_len(), (1, 0));
        state.undo()?;
        assert_eq!(state.data(), &data);
    }
    Ok(())
}
#[test]
fn indent_outdent_and_collapsed_ids() -> Result {
    let mut data = fixture();
    data.node_data
        .children
        .get_mut(0)
        .ok_or("fixture")?
        .expanded = false;
    let mut state = MindMapState::new(data.clone())?;
    let operation =
        plugins::command_operation(&data, &["css".into()], Command::Outdent, "New node")?;
    state.apply(operation)?;
    assert_eq!(state.parent_id("css"), Some("root"));
    assert!(!state.node("design").ok_or("design")?.expanded);
    state.undo()?;
    assert_eq!(state.data(), &data);
    Ok(())
}
#[test]
fn rejected_and_readonly_do_not_damage_state() -> Result {
    let mut state = MindMapState::new(fixture())?;
    let before = state.data().clone();
    state.set_read_only(true);
    let operation =
        plugins::command_operation(&before, &["build".into()], Command::Child, "New node")?;
    assert_eq!(state.apply(operation), Err(MindMapError::ReadOnly));
    assert_eq!(state.data(), &before);
    assert_eq!(state.history_len(), (0, 0));
    assert!(plugins::command_operation(&before, &[], Command::Child, "New node").is_err());
    assert!(
        plugins::command_operation(&before, &["css".into()], Command::Indent, "New node").is_err()
    );
    Ok(())
}
#[test]
fn plaintext_unicode_escapes_and_multiple_roots() -> Result {
    let data = text::from_plaintext("- 中文\\nline\n  - C:\\\\temp\n- Rust\n")?;
    assert_eq!(data.node_data.topic, "Root");
    assert_eq!(data.node_data.children.len(), 2);
    let plain = text::to_plaintext(&data)?;
    let round = text::from_plaintext(&plain)?;
    assert_eq!(round.node_data.topic, "Root");
    assert_eq!(
        round.node_data.children.first().ok_or("child")?.topic,
        "中文\nline"
    );
    for invalid in [
        " - bad",
        "  - missing root",
        "- a\n    - skip",
        "- > [^a] -> [^b]",
        "- a[^ref]",
        "- }:2 summary",
        "- a\\q",
        "- a {\"color\":\"red\"}",
    ] {
        assert!(text::from_plaintext(invalid).is_err(), "{invalid}");
    }
    Ok(())
}
#[test]
fn plaintext_rejects_lossy_fields() -> Result {
    let mut data = fixture();
    data.node_data.note = Some("saved only in JSON".into());
    assert!(text::to_plaintext(&data).is_err());
    Ok(())
}
#[test]
fn drag_threshold_four_layout_order_and_edge_pan() -> Result {
    assert!(!gesture::is_drag(
        Point::default(),
        Point { x: 3.0, y: 3.0 }
    ));
    assert!(gesture::is_drag(Point::default(), Point { x: 3.0, y: 4.0 }));
    let rect = HitRect {
        x: 100.0,
        y: 100.0,
        width: 100.0,
        height: 60.0,
    };
    for layout in [
        LayoutDirection::Left,
        LayoutDirection::Right,
        LayoutDirection::Side,
        LayoutDirection::Down,
    ] {
        let before = Point { x: 101.0, y: 101.0 };
        let after = Point { x: 199.0, y: 159.0 };
        assert_eq!(
            gesture::drop_placement("target", rect, before, layout, 1.25)?,
            Placement::Before("target".into())
        );
        assert_eq!(
            gesture::drop_placement("target", rect, after, layout, 1.25)?,
            Placement::After("target".into())
        );
        assert_eq!(
            gesture::drop_placement("target", rect, Point { x: 150.0, y: 130.0 }, layout, 1.25)?,
            Placement::In("target".into())
        );
    }
    assert!(gesture::edge_pan(rect, Point { x: 101.0, y: 101.0 })?.x > 0.0);
    assert!(
        gesture::drop_placement("a", rect, Point::default(), LayoutDirection::Down, f64::NAN)
            .is_err()
    );
    Ok(())
}
#[test]
fn xmind_archive_independent_readback() -> Result {
    let mut data = fixture();
    data.node_data.note = Some("中文 note".into());
    data.node_data
        .children
        .get_mut(0)
        .ok_or("fixture")?
        .expanded = false;
    let original = data.clone();
    let file = export::xmind(&data)?;
    assert_eq!(data, original);
    let out =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/plugin-artifacts");
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("fixture.xmind"), &file.bytes)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(file.bytes))?;
    assert_eq!(archive.len(), 3);
    let mut content = String::new();
    archive
        .by_name("content.json")?
        .read_to_string(&mut content)?;
    let parsed: serde_json::Value = serde_json::from_str(&content)?;
    assert_eq!(
        parsed
            .pointer("/0/rootTopic/title")
            .and_then(|v| v.as_str()),
        Some("Rust 插件工作台")
    );
    assert_eq!(
        parsed
            .pointer("/0/rootTopic/children/attached/0/branch")
            .and_then(|v| v.as_str()),
        Some("folded")
    );
    assert_eq!(
        parsed
            .pointer("/0/rootTopic/children/attached/0/children/attached/0/title")
            .and_then(|v| v.as_str()),
        Some("CSS 变量")
    );
    let mut manifest = String::new();
    archive
        .by_name("manifest.json")?
        .read_to_string(&mut manifest)?;
    let manifest: serde_json::Value = serde_json::from_str(&manifest)?;
    assert!(manifest.pointer("/file-entries/content.json").is_some());
    Ok(())
}
#[test]
fn static_html_escaping_sink_errors_and_image_validation() -> Result {
    let svg = export::ExportFile {
        name: "a.svg".into(),
        mime: "image/svg+xml".into(),
        bytes: b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>".to_vec(),
    };
    let html = export::static_html("</title><script>bad</script>", &svg)?;
    let text = String::from_utf8(html.bytes)?;
    assert!(!text.contains("<script>"));
    assert!(text.contains("&lt;/title&gt;"));
    assert!(
        svg.save_with(|_| Err(MindMapError::InvalidOperation("disk full".into())))
            .is_err()
    );
    assert!(
        export::ImageOptions {
            scale: f64::NAN,
            ..Default::default()
        }
        .validate()
        .is_err()
    );
    Ok(())
}
#[test]
fn labels_default_to_english_and_cover_every_command() {
    let labels = Labels::default();
    assert_eq!(labels.new_topic, "New node");
    let mut seen = std::collections::BTreeSet::new();
    for command in [
        Command::Child,
        Command::SiblingAfter,
        Command::SiblingBefore,
        Command::Parent,
        Command::Delete,
        Command::Duplicate,
        Command::Fold,
        Command::Indent,
        Command::Outdent,
        Command::Undo,
        Command::Redo,
    ] {
        let text = labels.command(command);
        assert!(text.is_ascii() && !text.is_empty());
        assert!(seen.insert(text.to_string()), "duplicate label {text}");
    }
    let custom = Labels {
        undo: "撤销".into(),
        ..Labels::default()
    };
    assert_eq!(custom.command(Command::Undo), "撤销");
}
#[test]
fn malicious_export_input_is_validated() {
    let invalid = MindMapData::new(Node::new("", "bad"));
    assert!(export::xmind(&invalid).is_err());
}
#[test]
fn drag_core_cycle_rejected_atomically() -> Result {
    let mut state = MindMapState::new(fixture())?;
    let before = state.data().clone();
    assert_eq!(
        state.apply(Operation::Move {
            ids: vec!["design".into()],
            placement: Placement::In("css".into())
        }),
        Err(MindMapError::AncestorMove)
    );
    assert_eq!(state.data(), &before);
    Ok(())
}
