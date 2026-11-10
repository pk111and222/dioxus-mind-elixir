use crate::{
    MindMapError, MindMapHandle, Node, Operation, Placement, STYLESHEET,
    plugins::{self, Capability, Command, Locale},
};
use dioxus::prelude::*;
use std::collections::BTreeSet;

/// Shared document outline. Edits and moves use the same history as the map.
/// Browsing a collapsed branch here does not expand the map automatically.
#[component]
pub fn Outline(
    handle: MindMapHandle,
    #[props(default)] locale: Locale,
    #[props(default)] class: String,
    #[props(default)] style: String,
    #[props(default)] on_error: EventHandler<MindMapError>,
) -> Element {
    let mut focus = use_signal(|| None::<String>);
    let dragging = use_signal(|| None::<String>);
    let hidden = use_signal(BTreeSet::<String>::new);
    let data = match handle.document() {
        Ok(data) => data,
        Err(error) => return rsx! { p {role:"alert", "{error}"} },
    };
    let selection = match handle.selection() {
        Ok(ids) => ids,
        Err(error) => return rsx! {p {role:"alert", "{error}"}},
    };
    if !handle.has_plugin(Capability::Outline) {
        return rsx! {};
    }
    let root = focus()
        .as_ref()
        .and_then(|id| data.node_data.find(id))
        .unwrap_or(&data.node_data)
        .clone();
    let mut crumbs = vec![data.node_data.clone()];
    if root.id != data.node_data.id {
        let mut path = Vec::new();
        let mut node = &root;
        while let Some(parent) = plugins::parent(&data.node_data, &node.id) {
            path.push(node.clone());
            node = parent;
        }
        path.reverse();
        crumbs.extend(path);
    }
    let mut rows = Vec::new();
    // Outline's expansion belongs to its own view; map collapse is not a DOM lookup gate.
    fn flatten(
        node: &Node,
        depth: usize,
        rows: &mut Vec<(Node, usize)>,
        hidden: &BTreeSet<String>,
    ) {
        rows.push((node.clone(), depth));
        if !hidden.contains(&node.id) {
            for child in &node.children {
                flatten(child, depth + 1, rows, hidden);
            }
        }
    }
    flatten(&root, 0, &mut rows, &hidden.read());
    rsx! {
        style {"{STYLESHEET}"}
        section {class:"dme-outline {class}", style, aria_label: if locale == Locale::Zh {"大纲"} else {"Outline"},
            nav {class:"dme-breadcrumbs", aria_label: if locale == Locale::Zh {"路径"} else {"Breadcrumbs"},
                for crumb in crumbs { button {r#type:"button", onclick:move |_| focus.set(Some(crumb.id.clone())), "{crumb.topic}"} }
            }
            div {role:"tree",
                for (node,depth) in rows {
                    OutlineRow {key:"{node.id}", selected:selection.contains(&node.id), node, depth, handle, locale, focus, dragging, hidden, on_error}
                }
            }
        }
    }
}
#[component]
fn OutlineRow(
    node: Node,
    depth: usize,
    selected: bool,
    handle: MindMapHandle,
    locale: Locale,
    mut focus: Signal<Option<String>>,
    mut dragging: Signal<Option<String>>,
    mut hidden: Signal<BTreeSet<String>>,
    on_error: EventHandler<MindMapError>,
) -> Element {
    let owner = dioxus::core::current_scope_id().0;
    let mut editing = use_signal(|| false);
    let mut draft = use_signal(|| node.topic.clone());
    let mut composing = use_signal(|| false);
    let mut ended = use_signal(|| false);
    let mut focused = use_signal(|| false);
    let is_hidden = hidden.read().contains(&node.id);
    let mut edit_revision = use_signal(|| None::<u64>);
    use_effect(move || {
        if editing()
            && !matches!((handle.revision(), *edit_revision.read()), (Ok(current),Some(start)) if current==start)
        {
            editing.set(false);
        }
    });
    use_effect(move || {
        if !matches!(handle.owns_editor(owner), Ok(true))
            || !matches!(handle.is_read_only(), Ok(false))
        {
            editing.set(false);
        }
    });
    let select_id = node.id.clone();
    let edit_node = node.clone();
    let fold_id = node.id.clone();
    let toggle_id = node.id.clone();
    let key_node = node.clone();
    let drag_id = node.id.clone();
    let drop_id = node.id.clone();
    let input_id = node.id.clone();
    let report = move |result: Result<(), MindMapError>| {
        if let Err(error) = result {
            on_error.call(error);
        }
    };
    let start = use_callback(move |_: ()| {
        report((|| {
            if handle.is_read_only()? {
                return Err(MindMapError::ReadOnly);
            }
            handle.select(std::slice::from_ref(&edit_node.id))?;
            handle.claim_editor(owner)?;
            edit_revision.set(Some(handle.revision()?));
            draft.set(edit_node.topic.clone());
            editing.set(true);
            Ok(())
        })());
    });
    rsx! {
        div {class:"dme-outline-row", role:"treeitem", tabindex:"0", "data-outline-id":"{node.id}", "data-selected":selected,
            style:format!("--dme-outline-depth:{depth}"), aria_level:depth+1, aria_selected:selected,
            draggable: handle.has_plugin(Capability::DragDrop) && !matches!(handle.is_read_only(),Ok(true)),
            ondragstart:move |_| { dragging.set(Some(drag_id.clone())); },
            ondragover:move |event| event.prevent_default(),
            ondrop:move |event| {
                event.prevent_default(); let source=dragging.peek().clone(); dragging.set(None);
                if let Some(source)=source { if source != drop_id {
                    report(handle.apply(Operation::Move {ids:vec![source],placement:Placement::In(drop_id.clone())}).map(|_| ()));
                }}
            },
            ondragend:move |_| dragging.set(None),
            onclick:move |event| {event.stop_propagation(); report(handle.select(std::slice::from_ref(&select_id)));},
            ondoubleclick:move |_| start.call(()),
            onkeydown:move |event| {
                if event.is_composing() || editing() {return;}
                if event.key()==Key::F2 {event.prevent_default(); start.call(()); return;}
                let command=match event.key() {
                    Key::Tab if event.modifiers().shift()=>Some(Command::Outdent),Key::Tab=>Some(Command::Indent),
                    Key::Enter if event.modifiers().ctrl()||event.modifiers().meta()=>Some(Command::Parent),
                    Key::Enter if event.modifiers().shift()=>Some(Command::SiblingBefore),Key::Enter=>Some(Command::SiblingAfter),
                    Key::Delete|Key::Backspace=>Some(Command::Delete),_=>None,
                };
                if let Some(command)=command {event.prevent_default();report((||{handle.select(std::slice::from_ref(&key_node.id))?;plugins::execute(handle,command,locale)})());}
            },
            button {class:"dme-outline-fold",r#type:"button",disabled:node.children.is_empty(), aria_expanded:!is_hidden,
                aria_label:if locale==Locale::Zh {"显示或隐藏大纲子节点"} else {"Toggle outline children"},
                onclick:move |event| {event.stop_propagation();if is_hidden {hidden.write().remove(&toggle_id);} else {hidden.write().insert(toggle_id.clone());}}, if is_hidden {"▸"} else {"▾"}
            }
            if editing() {
                textarea {class:"dme-outline-editor", rows:"2", aria_label:if locale==Locale::Zh {"编辑大纲节点"} else {"Edit outline node"},value:node.topic.clone(),autofocus:true,
                    onmounted:move |event| async move {if let Err(error)=event.data().set_focus(true).await {if !*focused.peek() {on_error.call(MindMapError::Measurement(error.to_string()));}}},
                    onfocus:move |_| focused.set(true),
                    oninput:move |event| draft.set(event.value()),
                    oncompositionstart:move |_| {composing.set(true);ended.set(false);},oncompositionend:move |_| {composing.set(false);ended.set(true);},onkeyup:move |_| ended.set(false),
                    onkeydown:move |event| {
                        event.stop_propagation(); if composing() || ended() || event.is_composing() {return;}
                        if event.key()==Key::Escape {event.prevent_default();editing.set(false);}
                        else if event.key()==Key::Enter && !event.modifiers().shift() {event.prevent_default();editing.set(false);
                            if !draft.peek().trim().is_empty() && *draft.peek()!=node.topic {
                                report(handle.apply(Operation::SetTopic{id:input_id.clone(),topic:draft.peek().clone()}).map(|_|()));
                            }
                        }
                    },onblur:move |_| {focused.set(false);editing.set(false);}
                }
            } else {span {class:"dme-outline-label", "{node.topic}"}}
            button {class:"dme-outline-action",r#type:"button",title:if locale==Locale::Zh {"聚焦此分支"} else {"Focus branch"},onclick:move |event| {event.stop_propagation();focus.set(Some(fold_id.clone()));},"↗"}
        }

    }
}

/// Optional toolbar. Host applications can also render their own command menus.
#[component]
pub fn Toolbar(
    handle: MindMapHandle,
    #[props(default)] locale: Locale,
    #[props(default)] class: String,
    #[props(default)] style: String,
    #[props(default)] on_error: EventHandler<MindMapError>,
) -> Element {
    if !handle.has_plugin(Capability::Toolbar) {
        return rsx! {};
    }
    let commands = match handle.plugin_commands() {
        Ok(commands) => commands,
        Err(error) => return rsx! {p{role:"alert","{error}"}},
    };
    rsx! { style {"{STYLESHEET}"} div {class:"dme-toolbar {class}",style,role:"toolbar", aria_label:if locale==Locale::Zh {"脑图工具栏"} else {"Mind map toolbar"},
        for command in commands {button {r#type:"button", disabled:matches!(handle.is_read_only(),Ok(true)), onclick:move |_| {if let Err(error)=plugins::execute(handle,command,locale){on_error.call(error);}},"{locale.label(command)}"}}
    }}
}
