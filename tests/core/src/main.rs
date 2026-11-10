//! Standalone Web/Desktop core application; no optional plugins installed.
use dioxus::prelude::*;
use dioxus_mind_elixir::{
    Alignment, EditableMindMap, LayoutDirection, MindMapError, Node, Operation, Placement,
    Viewport, use_mind_map,
};
use mind_elixir_core_tests::fixture;

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let handle = use_mind_map(fixture());
    let mut message = use_signal(String::new);
    let mut dark = use_signal(|| false);
    let mut spacious = use_signal(|| false);
    let mut second = use_signal(|| true);
    let mut alignment = use_signal(|| Alignment::Root);
    let mut viewport = use_signal(Viewport::default);
    let mut center_revision = use_signal(|| 0u64);
    let mut serial = use_signal(|| 0u64);
    let events = use_signal(Vec::<String>::new);
    let mut json = use_signal(String::new);
    let Ok(handle) = handle else {
        return rsx! { p { role: "alert", "Invalid fixture" } };
    };
    use_hook(move || {
        if let Err(error) = handle.set_listener(move |event| {
            let mut events = events;
            let mut items = events.write();
            items.push(format!("{event:?}"));
            if items.len() > 12 {
                items.remove(0);
            }
        }) {
            message.set(error.to_string());
        }
    });
    let mut report = move |result: Result<(), MindMapError>| {
        message.set(match result {
            Ok(()) => String::new(),
            Err(error) => error.to_string(),
        });
    };
    let selected = handle.selection();
    let target = match selected {
        Ok(ids) => ids.first().cloned().unwrap_or_else(|| "root".into()),
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let document = match handle.document() {
        Ok(data) => data,
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let readonly = match handle.is_read_only() {
        Ok(value) => value,
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let theme = if dark() { "dark" } else { "light" };
    let density = if spacious() { "spacious" } else { "normal" };
    let add_target = target.clone();
    let remove_target = target.clone();
    let copy_target = target.clone();
    let parent_target = target.clone();
    let move_target = target.clone();
    let toggle_target = target.clone();
    rsx! {
        style { {include_str!("../core.css")} }
        main {
            header { p { "DIOXUS MIND ELIXIR / CORE LAB" } h1 { "核心能力试验台" } p { "双击或 F2 编辑 · Enter / Tab 提交 · Shift+Enter 换行 · Esc 取消 · 拖动画布平移 · Ctrl/⌘ 滚轮缩放 · Home 重置视口" } }
            nav { aria_label: "Core controls",
                for (label, direction) in [("LEFT", LayoutDirection::Left), ("RIGHT", LayoutDirection::Right), ("SIDE", LayoutDirection::Side), ("DOWN", LayoutDirection::Down)] {
                    button { onclick: move |_| report(handle.apply(Operation::SetLayout(direction)).map(|_| ())), "{label}" }
                }
                button { onclick: move |_| dark.toggle(), "主题" }
                button { onclick: move |_| spacious.toggle(), "CSS 间距/字体" }
                button { onclick: move |_| report(handle.apply(Operation::SetCompact(!document.compact)).map(|_| ())), "紧凑" }
                button { onclick: move |_| alignment.set(if alignment() == Alignment::Root { Alignment::Nodes } else { Alignment::Root }), "对齐" }
                button { onclick: move |_| { let mut next = viewport(); report(next.zoom_at(next.scale * 1.25, dioxus_mind_elixir::Point { x: 400.0, y: 220.0 })); viewport.set(next); }, "Zoom +" }
                button { onclick: move |_| { let mut next = viewport(); report(next.zoom_at(next.scale / 1.25, dioxus_mind_elixir::Point { x: 400.0, y: 220.0 })); viewport.set(next); }, "Zoom −" }
                button { onclick: move |_| { let next = center_revision.peek().saturating_add(1); center_revision.set(next); }, "居中" }
                button { onclick: move |_| viewport.set(Viewport::default()), "视口重置" }
                button { onclick: move |_| report(handle.undo()), "Undo" }
                button { onclick: move |_| report(handle.redo()), "Redo" }
                button { onclick: move |_| report(handle.set_read_only(!readonly)), if readonly { "允许编辑" } else { "只读" } }
                button { onclick: move |_| second.toggle(), if second() { "卸载第二实例" } else { "挂载第二实例" } }
                button { onclick: move |_| report(handle.refresh(fixture())), "Refresh" }
            }
            section { class: "operations",
                span { id: "selected", "选中：{target}" }
                button { onclick: move |_| {
                    let next = serial.peek().saturating_add(1); serial.set(next);
                    report(handle.apply(Operation::Insert { node: Node::new(format!("new-{next}"), "新节点"), placement: Placement::In(add_target.clone()) }).map(|_| ()));
                }, "添加子节点" }
                button { onclick: move |_| report(handle.apply(Operation::Remove(vec![remove_target.clone()])).map(|_| ())), "删除" }
                button { onclick: move |_| report(handle.apply(Operation::Copy { ids: vec![copy_target.clone()], placement: Placement::In("root".into()) }).map(|_| ())), "复制到根" }
                button { onclick: move |_| {
                    let next = serial.peek().saturating_add(1); serial.set(next);
                    report(handle.apply(Operation::InsertParent { target: parent_target.clone(), parent: Node::new(format!("parent-{next}"), "新父节点") }).map(|_| ()));
                }, "插入父节点" }
                button { onclick: move |_| report(handle.apply(Operation::Move { ids: vec![move_target.clone()], placement: Placement::In("build".into()) }).map(|_| ())), "移动到构建" }
                button { onclick: move |_| {
                    let result = (|| {
                        let data = handle.document()?;
                        let node = data.node_data.find(&toggle_target).ok_or_else(|| MindMapError::NodeNotFound(toggle_target.clone()))?;
                        handle.apply(Operation::SetExpanded { id: toggle_target.clone(), expanded: !node.expanded })?; Ok(())
                    })(); report(result);
                }, "折叠/展开" }
            }
            p { id: "status", role: "status", "{message}" }
            section { class: "primary {theme} {density}", id: "primary", EditableMindMap { handle, viewport: Some(viewport), center_revision: center_revision(), alignment: alignment(), on_error: move |error| report(Err(error)) } }
            if second() { section { class: "secondary", id: "secondary", EditableMindMap { handle, on_error: move |error| report(Err(error)) } } }
            details { summary { "JSON 导入 / 导出" }
                button { onclick: move |_| { match handle.document().and_then(|data| data.to_json()) { Ok(value) => json.set(value), Err(error) => report(Err(error)) } }, "导出 JSON" }
                button { onclick: move |_| report(dioxus_mind_elixir::MindMapData::from_json(&json()).and_then(|data| handle.refresh(data))), "导入 JSON" }
                textarea { aria_label: "Document JSON", value: json, oninput: move |event| json.set(event.value()) }
            }
            details { summary { "操作事件（最近 12 条）" } pre { for item in events.read().iter() { "{item}\n" } } }
        }
    }
}
