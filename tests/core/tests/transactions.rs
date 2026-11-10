//! Core regression tests use only the published API, with no renderer/plugins.
use dioxus_mind_elixir::{
    CoreEvent, LayoutDirection, MindMapData, MindMapError, MindMapState, Node, NodeDirection,
    Operation, OperationOptions, Placement, Point, Viewport,
};
use mind_elixir_core_tests::fixture;
use std::collections::BTreeSet;

fn ids(nodes: &[Node]) -> Vec<String> {
    nodes.iter().map(|node| node.id.clone()).collect()
}
fn node<'a>(state: &'a MindMapState, id: &str) -> Result<&'a Node, MindMapError> {
    state
        .node(id)
        .ok_or_else(|| MindMapError::NodeNotFound(id.into()))
}
fn all_ids(root: &Node) -> BTreeSet<String> {
    let mut pending = vec![root];
    let mut ids = BTreeSet::new();
    while let Some(node) = pending.pop() {
        ids.insert(node.id.clone());
        pending.extend(&node.children);
    }
    ids
}
#[test]
fn strict_json_roundtrip_preserves_supported_fields() -> Result<(), MindMapError> {
    let data = MindMapData::from_json(include_str!("../fixtures/plain.json"))?;
    assert_eq!(data.direction, LayoutDirection::Side);
    assert!(data.compact);
    assert!(data.node_data.expanded);
    assert_eq!(data, MindMapData::from_json(&data.to_json()?)?);
    assert!(!data.to_json()?.contains("selection"));
    Ok(())
}
#[test]
fn unsupported_fields_directions_ids_and_trailing_data_are_errors() {
    for field in [
        "image",
        "arrows",
        "summaries",
        "dangerouslySetInnerHTML",
        "unknown",
    ] {
        let input = format!(r#"{{"nodeData":{{"id":"r","topic":"x","{field}":{{}}}}}}"#);
        assert!(
            matches!(MindMapData::from_json(&input), Err(MindMapError::InvalidData { path, .. }) if path.contains("nodeData"))
        );
    }
    assert!(
        MindMapData::from_json(r#"{"nodeData":{"id":"r","topic":"x"},"direction":4}"#).is_err()
    );
    assert!(
        MindMapData::from_json(r#"{"nodeData":{"id":"r","topic":"x","direction":2}}"#).is_err()
    );
    assert!(MindMapData::from_json(r#"{"nodeData":{"id":"r","topic":"x"}} {}"#).is_err());
    assert!(matches!(
        MindMapData::from_json(
            r#"{"nodeData":{"id":"r","topic":"x","children":[{"id":"r","topic":"y"}]}}"#
        ),
        Err(MindMapError::DuplicateNodeId(_))
    ));
}
#[test]
fn failed_transactions_leave_every_runtime_invariant_unchanged() -> Result<(), MindMapError> {
    let mut state = MindMapState::new(fixture())?;
    state.select(&["css".into()])?;
    let data = state.data().clone();
    let selection = state.selection().to_vec();
    let revision = state.revision();
    for operation in [
        Operation::Move {
            ids: vec!["design".into()],
            placement: Placement::In("css".into()),
        },
        Operation::Insert {
            node: Node::new("root", "duplicate"),
            placement: Placement::In("build".into()),
        },
        Operation::Remove(vec!["root".into()]),
        Operation::Remove(vec!["css".into(), "missing".into()]),
        Operation::Move {
            ids: vec!["css".into()],
            placement: Placement::Before("root".into()),
        },
        Operation::InsertParent {
            target: "css".into(),
            parent: Node::new("build", "duplicate"),
        },
        Operation::SetTopic {
            id: "css".into(),
            topic: " \n".into(),
        },
    ] {
        assert!(state.apply(operation).is_err());
        assert_eq!(state.data(), &data);
        assert_eq!(state.selection(), &selection);
        assert_eq!(state.revision(), revision);
        assert_eq!(state.history_len(), (0, 0));
        assert_eq!(state.parent_id("css"), Some("design"));
    }
    Ok(())
}
#[test]
fn add_to_collapsed_parent_is_one_undo_and_restores_selection() -> Result<(), MindMapError> {
    let mut state = MindMapState::new(fixture())?;
    state.apply(Operation::SetExpanded {
        id: "design".into(),
        expanded: false,
    })?;
    state.select(&["design".into()])?;
    let before = state.data().clone();
    let result = state.apply(Operation::Insert {
        node: Node::new("new", "New"),
        placement: Placement::In("design".into()),
    })?;
    assert!(node(&state, "design")?.expanded);
    assert_eq!(state.history_len(), (2, 0));
    assert!(matches!(
        result.events.first(),
        Some(CoreEvent::Operation(_))
    ));
    assert!(matches!(result.events.get(1), Some(CoreEvent::Selection(ids)) if ids == &["new"]));
    state.undo()?;
    assert_eq!(state.data(), &before);
    assert_eq!(state.selection(), &["design"]);
    state.redo()?;
    assert!(node(&state, "design")?.expanded);
    assert_eq!(state.selection(), &["new"]);
    Ok(())
}
#[test]
fn ancestor_dedup_and_batch_after_keep_traversal_order_and_ids() -> Result<(), MindMapError> {
    let mut state = MindMapState::new(fixture())?;
    let old_ids = all_ids(&state.data().node_data);
    state.apply(Operation::Move {
        ids: vec!["history".into(), "css".into(), "design".into()],
        placement: Placement::After("build".into()),
    })?;
    assert_eq!(
        ids(&state.data().node_data.children),
        ["build", "design", "history"]
    );
    assert_eq!(all_ids(&state.data().node_data), old_ids);
    assert_eq!(state.parent_id("css"), Some("design"));
    state.undo()?;
    state.apply(Operation::Remove(vec![
        "design".into(),
        "css".into(),
        "design".into(),
    ]))?;
    assert!(state.node("css").is_none());
    assert!(state.node("design").is_none());
    assert_eq!(state.selection(), &["root"]);
    Ok(())
}
#[test]
fn copies_regenerate_entire_subtree_without_mutating_original() -> Result<(), MindMapError> {
    let mut state = MindMapState::new(fixture())?;
    state.apply(Operation::SetExpanded {
        id: "build".into(),
        expanded: false,
    })?;
    let old_ids = all_ids(&state.data().node_data);
    let original = node(&state, "design")?.clone();
    let result = state.apply(Operation::Copy {
        ids: vec!["design".into(), "css".into()],
        placement: Placement::In("build".into()),
    })?;
    let copied = node(&state, result.ids.first().ok_or(MindMapError::NoHistory)?)?;
    assert_eq!(copied.topic, original.topic);
    assert_eq!(copied.children.len(), original.children.len());
    assert!(all_ids(copied).is_disjoint(&old_ids));
    assert_eq!(node(&state, "design")?, &original);
    assert!(node(&state, "build")?.expanded);
    state.undo()?;
    assert!(!node(&state, "build")?.expanded);
    assert_eq!(all_ids(&state.data().node_data), old_ids);
    Ok(())
}
#[test]
fn insert_parent_siblings_and_side_assignment_are_persistent() -> Result<(), MindMapError> {
    let mut data = fixture();
    data.direction = LayoutDirection::Side;
    data.node_data
        .children
        .get_mut(0)
        .ok_or(MindMapError::NoHistory)?
        .direction = Some(NodeDirection::Right);
    let mut state = MindMapState::new(data)?;
    assert_eq!(
        node(&state, "design")?.direction,
        Some(NodeDirection::Right)
    );
    assert_eq!(node(&state, "build")?.direction, Some(NodeDirection::Left));
    state.apply(Operation::Insert {
        node: Node::new("sibling", "S"),
        placement: Placement::Before("design".into()),
    })?;
    assert_eq!(
        node(&state, "sibling")?.direction,
        Some(NodeDirection::Right)
    );
    state.apply(Operation::InsertParent {
        target: "design".into(),
        parent: Node::new("wrapper", "Parent"),
    })?;
    assert_eq!(state.parent_id("design"), Some("wrapper"));
    assert_eq!(
        node(&state, "wrapper")?.direction,
        Some(NodeDirection::Right)
    );
    Ok(())
}
#[test]
fn noops_silent_redo_truncation_and_refresh_baseline() -> Result<(), MindMapError> {
    let mut state = MindMapState::new(fixture())?;
    assert!(
        !state
            .apply(Operation::SetLayout(LayoutDirection::Right))?
            .changed
    );
    assert!(
        state
            .apply(Operation::Remove(Vec::new()))?
            .events
            .is_empty()
    );
    state.apply(Operation::SetTopic {
        id: "css".into(),
        topic: "Changed".into(),
    })?;
    state.undo()?;
    let result = state.apply_with(
        Operation::SetExpanded {
            id: "design".into(),
            expanded: false,
        },
        OperationOptions { silent: true },
    )?;
    assert!(result.changed);
    assert!(result.events.is_empty());
    assert_eq!(state.history_len(), (0, 0));
    state.apply(Operation::SetTopic {
        id: "css".into(),
        topic: "New edit".into(),
    })?;
    state.refresh(fixture())?;
    assert_eq!(state.history_len(), (0, 0));
    assert!(state.selection().is_empty());
    assert_eq!(state.undo(), Err(MindMapError::NoHistory));
    Ok(())
}
#[test]
fn readonly_covers_mutations_history_and_refresh_but_allows_selection() -> Result<(), MindMapError>
{
    let mut state = MindMapState::new(fixture())?;
    state.apply(Operation::SetCompact(true))?;
    state.set_read_only(true);
    assert_eq!(
        state.apply(Operation::SetCompact(false)),
        Err(MindMapError::ReadOnly)
    );
    assert_eq!(state.undo(), Err(MindMapError::ReadOnly));
    assert_eq!(state.redo(), Err(MindMapError::ReadOnly));
    assert_eq!(state.refresh(fixture()), Err(MindMapError::ReadOnly));
    state.select(&["css".into()])?;
    assert_eq!(state.selection(), &["css"]);
    Ok(())
}
#[test]
fn zoom_anchor_and_invalid_viewport_are_transactional() -> Result<(), MindMapError> {
    let mut viewport = Viewport::default();
    viewport.pan(Point { x: 12.0, y: -20.0 })?;
    let anchor = Point { x: 300.0, y: 150.0 };
    let before = viewport.to_canvas(anchor)?;
    viewport.zoom_at(2.5, anchor)?;
    assert_eq!(viewport.to_canvas(anchor)?, before);
    let saved = viewport;
    assert!(
        viewport
            .pan(Point {
                x: f64::NAN,
                y: 0.0
            })
            .is_err()
    );
    assert_eq!(viewport, saved);
    assert!(viewport.zoom_at(f64::INFINITY, anchor).is_err());
    assert_eq!(viewport, saved);
    viewport.center(Point { x: 100.0, y: 100.0 }, Point { x: 800.0, y: 600.0 })?;
    assert_eq!(
        viewport.to_canvas(Point { x: 400.0, y: 300.0 })?,
        Point { x: 100.0, y: 100.0 }
    );
    Ok(())
}
#[test]
fn bounded_history_and_deep_insert_rollback() -> Result<(), MindMapError> {
    let mut state = MindMapState::new(fixture())?;
    state.set_history_limit(2);
    for i in 0..4 {
        state.apply(Operation::SetTopic {
            id: "css".into(),
            topic: format!("Edit {i}"),
        })?;
    }
    assert_eq!(state.history_len(), (2, 0));
    let before = state.data().clone();
    let mut deep = Node::new("leaf", "leaf");
    for i in 0..129 {
        deep = Node::new(format!("deep-{i}"), "level").with_children(vec![deep]);
    }
    assert_eq!(
        state.apply(Operation::Insert {
            node: deep,
            placement: Placement::In("root".into())
        }),
        Err(MindMapError::TreeTooDeep)
    );
    assert_eq!(state.data(), &before);
    assert_eq!(state.history_len(), (2, 0));
    Ok(())
}

#[test]
fn explicit_null_metadata_is_distinct_from_missing() -> Result<(), MindMapError> {
    let null = MindMapData::from_json(
        r#"{"nodeData":{"id":"r","topic":"Root","metadata":null},"meta":null}"#,
    )?;
    assert_eq!(null.node_data.metadata, Some(serde_json::Value::Null));
    assert_eq!(null.meta, Some(serde_json::Value::Null));
    assert_eq!(MindMapData::from_json(&null.to_json()?)?, null);
    Ok(())
}

#[test]
fn theme_variable_validation_keeps_previous_values_on_error() -> Result<(), MindMapError> {
    let mut theme = dioxus_mind_elixir::Theme::new("custom");
    theme.set_variable("--dme-font-size", "18px")?;
    assert!(theme.set_variable("font-size", "20px").is_err());
    assert!(
        theme
            .set_variable("--dme-font-size", "18px; color:red")
            .is_err()
    );
    assert_eq!(
        theme.variables().get("--dme-font-size").map(String::as_str),
        Some("18px")
    );
    Ok(())
}

#[test]
fn overflowing_coordinate_conversion_is_an_error() {
    let viewport = Viewport {
        translation: Point {
            x: -f64::MAX,
            y: 0.0,
        },
        scale: 0.1,
    };
    assert_eq!(
        viewport.to_canvas(Point {
            x: f64::MAX,
            y: 0.0
        }),
        Err(MindMapError::InvalidGeometry)
    );
}
