use crate::plugins::{
    self, Capability, Command, Locale,
    gesture::{self, HitRect},
};
use crate::{
    LayoutDirection, MindMapData, MindMapError, MindMapHandle, Node, NodeDirection, Operation,
    Point, STYLESHEET, Theme, Viewport,
    generate_branch::{branch, local},
    platform,
};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use std::{collections::BTreeMap, rc::Rc};

type Mounts = BTreeMap<String, Rc<MountedData>>;

/// Vertical alignment of the root and side groups.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Alignment {
    /// Center the root against the full branch group.
    #[default]
    Root,
    /// Align side groups at their first nodes.
    Nodes,
}

/// Properties of the read-only [`MindMap`] renderer.
#[derive(Props, Clone, PartialEq)]
pub struct MindMapProps {
    /// Validated persistent document, reactive without remounting the map.
    pub data: ReadSignal<MindMapData>,
    /// Host CSS class.
    #[props(default)]
    pub class: String,
    /// Host CSS, including inherited `--dme-*` variables.
    #[props(default)]
    pub style: String,
    /// Explicit invalidation for host animations/position-only changes.
    #[props(default)]
    pub measurement_revision: u64,
    /// Root alignment.
    #[props(default)]
    pub alignment: Alignment,
    /// Optional explicit theme; omitted preserves ancestor CSS variables.
    #[props(default)]
    pub theme: Option<Theme>,
    /// Optional per-view viewport control, separate from document history.
    #[props(default)]
    pub viewport: Option<Signal<Viewport>>,
    /// Increment to center the root in the visible container.
    #[props(default)]
    pub center_revision: u64,
    /// Reports recoverable errors.
    #[props(default)]
    pub on_error: EventHandler<MindMapError>,
    /// Increment to fit all visible nodes into the container (zoom ≤ 100%).
    #[props(default)]
    pub fit_revision: u64,
    /// Map a stored node image reference to a URL the renderer can load.
    #[props(default)]
    pub image_url: Option<Callback<String, String>>,
}

/// Render a reactive document without an editing controller.
#[component]
pub fn MindMap(props: MindMapProps) -> Element {
    rsx! { MapBody { document: props.data.read().clone(), class: props.class, style: props.style, measurement_revision: props.measurement_revision, alignment: props.alignment, theme: props.theme, viewport_control: props.viewport, center_revision: props.center_revision, on_error: props.on_error, fit_revision: props.fit_revision, image_url: props.image_url } }
}

/// Editable view of a shared checked document. Double-click a node to edit.
/// Enter/Tab commit, Shift+Enter inserts a line break, Escape cancels. Blur
/// cancels unless `blur_commits` is set. The draft is visible to the host via
/// [`MindMapHandle::pending_edit`]. Wheel pans; Ctrl/Meta+wheel zooms.
/// Viewport state belongs to this view, never to history.
#[component]
pub fn EditableMindMap(
    handle: MindMapHandle,
    #[props(default)] class: String,
    #[props(default)] style: String,
    #[props(default)] measurement_revision: u64,
    #[props(default)] alignment: Alignment,
    #[props(default)] theme: Option<Theme>,
    #[props(default)] viewport: Option<Signal<Viewport>>,
    #[props(default)] center_revision: u64,
    #[props(default)] on_error: EventHandler<MindMapError>,
    #[props(default)] locale: Locale,
    #[props(default)] fit_revision: u64,
    #[props(default)] image_url: Option<Callback<String, String>>,
    #[props(default)] blur_commits: bool,
) -> Element {
    let document = handle.document();
    match document {
        Ok(document) => {
            rsx! { MapBody { document, handle: Some(handle), locale, class, style, measurement_revision, alignment, theme, viewport_control: viewport, center_revision, on_error, fit_revision, image_url, blur_commits } }
        }
        Err(error) => rsx! { p { role: "alert", "{error}" } },
    }
}

#[derive(Clone, PartialEq)]
struct EditSession {
    id: String,
    original: String,
    revision: u64,
}
#[derive(Clone, PartialEq)]
struct NodeDrag {
    start: Point,
    current: Point,
    source: Option<String>,
    active: bool,
    revision: u64,
}
#[derive(Clone, Copy)]
struct ViewContext {
    mounts: Signal<Mounts>,
    resize: Signal<u64>,
    editor: Signal<Option<EditSession>>,
    handle: Option<MindMapHandle>,
    on_error: EventHandler<MindMapError>,
    view_id: usize,
    gesture: Signal<Option<NodeDrag>>,
    menu: Signal<Option<Point>>,
    locale: Locale,
    suppress_click: Signal<bool>,
    gesture_epoch: Signal<u64>,
    image_url: Option<Callback<String, String>>,
    blur_commits: bool,
}
impl ViewContext {
    fn resized(mut self) {
        let next = self.resize.peek().saturating_add(1);
        self.resize.set(next);
    }
    fn report(self, result: Result<(), MindMapError>) {
        if let Err(error) = result {
            self.on_error.call(error);
        }
    }
    fn select(self, id: &str) {
        if let Some(handle) = self.handle {
            self.report(handle.select(&[id.to_owned()]));
        }
    }
    fn click_select(self, id: &str, toggle: bool) {
        if let Some(handle) = self.handle {
            let result = (|| {
                let mut ids = if toggle && handle.has_plugin(Capability::MultiSelect) {
                    handle.selection()?
                } else {
                    Vec::new()
                };
                if let Some(index) = ids.iter().position(|item| item == id) {
                    ids.remove(index);
                } else {
                    ids.push(id.into());
                }
                handle.select(&ids)
            })();
            self.report(result);
        }
    }
    fn keyboard(self, event: KeyboardEvent, node: &Node) {
        if event.is_composing() {
            return;
        }
        let Some(handle) = self.handle else {
            return;
        };
        if !handle.has_plugin(Capability::Keyboard) {
            return;
        }
        let modifiers = event.modifiers();
        let primary = modifiers.ctrl() || modifiers.meta();
        let command = match event.key() {
            Key::Enter if primary => Some(Command::Parent),
            Key::Enter if modifiers.shift() => Some(Command::SiblingBefore),
            Key::Enter => Some(Command::SiblingAfter),
            Key::Tab => Some(Command::Child),
            Key::Delete | Key::Backspace => Some(Command::Delete),
            Key::Character(ref c) if primary && c.eq_ignore_ascii_case("z") => {
                Some(if modifiers.shift() {
                    Command::Redo
                } else {
                    Command::Undo
                })
            }
            Key::Character(ref c) if primary && c.eq_ignore_ascii_case("y") => Some(Command::Redo),
            _ => None,
        };
        if let Some(command) = command {
            event.prevent_default();
            event.stop_propagation();
            self.report((|| {
                if handle.selection()?.is_empty() {
                    handle.select(std::slice::from_ref(&node.id))?;
                }
                plugins::execute(handle, command, self.locale)
            })());
        } else if matches!(
            event.key(),
            Key::ArrowUp | Key::ArrowDown | Key::ArrowLeft | Key::ArrowRight
        ) {
            event.prevent_default();
            event.stop_propagation();
            let elements = self.mounts.peek().clone();
            let id = node.id.clone();
            let key = event.key();
            spawn(async move {
                let result = async {
                    let revision = handle.revision()?;
                    let origin = elements.get(&id).ok_or(MindMapError::StaleOperation)?;
                    let origin = platform::measure(origin).await?;
                    let x = origin.x + origin.width / 2.0;
                    let y = origin.y + origin.height / 2.0;
                    let mut best = None::<(f64, String)>;
                    for (candidate, element) in &elements {
                        if candidate == &id {
                            continue;
                        }
                        let rect = platform::measure(element).await?;
                        let dx = rect.x + rect.width / 2.0 - x;
                        let dy = rect.y + rect.height / 2.0 - y;
                        let (forward, cross) = match key {
                            Key::ArrowLeft => (-dx, dy.abs()),
                            Key::ArrowRight => (dx, dy.abs()),
                            Key::ArrowUp => (-dy, dx.abs()),
                            _ => (dy, dx.abs()),
                        };
                        if forward > 1.0 {
                            let score = forward + cross * 3.0;
                            if best.as_ref().is_none_or(|(previous, _)| score < *previous) {
                                best = Some((score, candidate.clone()));
                            }
                        }
                    }
                    if handle.revision()? != revision {
                        return Err(MindMapError::StaleOperation);
                    }
                    if let Some((_, id)) = best {
                        handle.claim_editor(self.view_id)?;
                        handle.select(&[id])?;
                    }
                    Ok(())
                }
                .await;
                self.report(result);
            });
        }
    }
    fn start_edit(mut self, node: &Node) {
        if let Some(handle) = self.handle {
            let result = (|| {
                if handle.is_read_only()? {
                    return Err(MindMapError::ReadOnly);
                }
                handle.select(std::slice::from_ref(&node.id))?;
                handle.begin_edit(self.view_id, &node.id, &node.topic)?;
                self.editor.set(Some(EditSession {
                    id: node.id.clone(),
                    original: node.topic.clone(),
                    revision: handle.revision()?,
                }));
                Ok(())
            })();
            self.report(result);
        }
    }
}

#[component]
fn MapBody(
    document: MindMapData,
    #[props(default)] handle: Option<MindMapHandle>,
    class: String,
    style: String,
    measurement_revision: u64,
    alignment: Alignment,
    #[props(default)] theme: Option<Theme>,
    #[props(default)] viewport_control: Option<Signal<Viewport>>,
    #[props(default)] center_revision: u64,
    on_error: EventHandler<MindMapError>,
    #[props(default)] locale: Locale,
    #[props(default)] fit_revision: u64,
    #[props(default)] image_url: Option<Callback<String, String>>,
    #[props(default)] blur_commits: bool,
) -> Element {
    let mounts = use_signal(Mounts::new);
    let canvas = use_signal(|| None::<Rc<MountedData>>);
    let scale_probe = use_signal(|| None::<Rc<MountedData>>);
    let mut container = use_signal(|| None::<Rc<MountedData>>);
    let resize = use_signal(|| 0u64);
    let editor = use_signal(|| None::<EditSession>);
    let mut gesture = use_signal(|| None::<NodeDrag>);
    let mut menu = use_signal(|| None::<Point>);
    let suppress_click = use_signal(|| false);
    let mut gesture_epoch = use_signal(|| 0u64);
    let context = ViewContext {
        mounts,
        resize,
        editor,
        handle,
        on_error,
        view_id: dioxus::core::current_scope_id().0,
        gesture,
        menu,
        locale,
        suppress_click,
        gesture_epoch,
        image_url,
        blur_commits,
    };
    use_context_provider(|| context);
    let local_viewport = use_signal(Viewport::default);
    let mut viewport = viewport_control.unwrap_or(local_viewport);
    let mut drag = use_signal(|| None::<Point>);
    let mut measurement_error = use_signal(|| None::<MindMapError>);
    let mut input = use_signal(|| (document.clone(), measurement_revision, theme.clone()));
    use_effect(use_reactive(
        (&document, &measurement_revision, &theme),
        move |(document, revision, theme)| {
            input.set((document, revision, theme));
        },
    ));
    let geometry = use_resource(move || async move {
        let (document, _, theme) = input.read().clone();
        let elements = mounts.read().clone();
        let element = canvas.read().clone();
        let probe = scale_probe.read().clone();
        let _ = resize();
        document.validate()?;
        let (Some(element), Some(probe)) = (element, probe) else {
            return Ok(Vec::new());
        };
        let scale = platform::measure(&probe).await?.width / 100.0;
        if !scale.is_finite() || scale <= 0.0 {
            return Err(MindMapError::InvalidGeometry);
        }
        let canvas_rect = platform::measure(&element).await?;
        let mut pending = vec![(
            &document.node_data,
            None::<String>,
            document.direction,
            None::<String>,
        )];
        let mut rectangles = BTreeMap::new();
        let mut edges = Vec::new();
        while let Some((node, parent, direction, color)) = pending.pop() {
            let Some(element) = elements.get(&node.id) else {
                return Ok(Vec::new());
            };
            rectangles.insert(
                node.id.clone(),
                local(platform::measure(element).await?, canvas_rect, scale),
            );
            let color = node.branch_color.clone().or(color);
            if let Some(parent_id) = parent {
                edges.push((parent_id, node.id.clone(), direction, color.clone()));
            }
            if node.expanded {
                for (index, child) in node.children.iter().enumerate().rev() {
                    let child_direction = if node.id == document.node_data.id
                        && document.direction == LayoutDirection::Side
                    {
                        match child.direction {
                            Some(NodeDirection::Left) => LayoutDirection::Left,
                            Some(NodeDirection::Right) => LayoutDirection::Right,
                            None => assigned_side(&document.node_data.children, &child.id),
                        }
                    } else {
                        direction
                    };
                    let child_color = if node.id == document.node_data.id {
                        theme
                            .as_ref()
                            .and_then(|theme| {
                                if theme.palette.is_empty() {
                                    None
                                } else {
                                    theme.palette.get(index % theme.palette.len()).cloned()
                                }
                            })
                            .or_else(|| color.clone())
                    } else {
                        color.clone()
                    };
                    pending.push((child, Some(node.id.clone()), child_direction, child_color));
                }
            }
        }
        // Desktop queries cross the renderer boundary. Reject mixed snapshots
        // if the canvas moved/resized/scaled during the batch. Resize/mount or
        // document changes already restart this scoped resource.
        let final_canvas = platform::measure(&element).await?;
        let final_scale = platform::measure(&probe).await?.width / 100.0;
        if final_canvas != canvas_rect || (final_scale - scale).abs() > 0.0001 {
            context.resized();
            return Ok(Vec::new());
        }
        // Merge paths with identical effective stroke, keeping DOM writes batched.
        let mut groups = BTreeMap::<Option<String>, String>::new();
        for (parent_id, child_id, direction, color) in edges {
            if let (Some(parent), Some(child)) =
                (rectangles.get(&parent_id), rectangles.get(&child_id))
            {
                let path = branch(
                    *parent,
                    *child,
                    direction,
                    parent_id == document.node_data.id,
                    canvas_rect.height / scale,
                );
                let group = groups.entry(color).or_default();
                group.push_str(&path);
                group.push(' ');
            }
        }
        Ok::<_, MindMapError>(groups.into_iter().collect::<Vec<_>>())
    });
    use_effect(move || {
        let error = match &*geometry.read() {
            Some(Err(error)) => Some(error.clone()),
            _ => None,
        };
        if *measurement_error.peek() != error {
            measurement_error.set(error.clone());
            if let Some(error) = error {
                on_error.call(error);
            }
        }
    });
    let mut center_request = use_signal(|| 0u64);
    use_effect(use_reactive((&center_revision,), move |(revision,)| {
        center_request.set(revision)
    }));
    let _center = use_resource(move || async move {
        let request = center_request();
        if request == 0 {
            return;
        }
        let root_id = input.read().0.node_data.id.clone();
        let elements = mounts.read().clone();
        let view_element = container.read().clone();
        let Some(root) = elements.get(&root_id) else {
            return;
        };
        let Some(view_element) = view_element else {
            return;
        };
        let before = *viewport.peek();
        let result = async {
            let root = platform::measure(root).await?;
            let view = platform::measure(&view_element).await?;
            let target = before.to_canvas(Point {
                x: root.x + root.width / 2.0 - view.x,
                y: root.y + root.height / 2.0 - view.y,
            })?;
            let mut next = before;
            next.center(
                target,
                Point {
                    x: view.width,
                    y: view.height,
                },
            )?;
            if *viewport.peek() == before {
                viewport.set(next);
            }
            Ok(())
        }
        .await;
        context.report(result);
    });
    let mut fit_request = use_signal(|| 0u64);
    use_effect(use_reactive((&fit_revision,), move |(revision,)| {
        fit_request.set(revision)
    }));
    let _fit = use_resource(move || async move {
        let request = fit_request();
        if request == 0 {
            return;
        }
        let elements = mounts.peek().clone();
        let view_element = container.peek().clone();
        let Some(view_element) = view_element else {
            return;
        };
        let before = *viewport.peek();
        let result = async {
            let view = platform::measure(&view_element).await?;
            let mut bounds: Option<(f64, f64, f64, f64)> = None;
            for element in elements.values() {
                let r = platform::measure(element).await?;
                bounds = Some(match bounds {
                    None => (r.x, r.y, r.x + r.width, r.y + r.height),
                    Some((x0, y0, x1, y1)) => (
                        x0.min(r.x),
                        y0.min(r.y),
                        x1.max(r.x + r.width),
                        y1.max(r.y + r.height),
                    ),
                });
            }
            let Some((x0, y0, x1, y1)) = bounds else {
                return Ok(());
            };
            let a = before.to_canvas(Point {
                x: x0 - view.x,
                y: y0 - view.y,
            })?;
            let b = before.to_canvas(Point {
                x: x1 - view.x,
                y: y1 - view.y,
            })?;
            let next = gesture::fit(
                a,
                b,
                Point {
                    x: view.width,
                    y: view.height,
                },
            )?;
            if *viewport.peek() == before {
                viewport.set(next);
            }
            Ok(())
        }
        .await;
        context.report(result);
    });
    // Deleting/replacing the edited node, or changing it elsewhere, cancels this
    // view's draft. No stale asynchronous result may restore that editor.
    use_effect(move || {
        let current = input.read();
        let session = context.editor.read().clone();
        if let Some(session) = session {
            let exists = current
                .0
                .node_data
                .find(&session.id)
                .is_some_and(|node| node.topic == session.original);
            let writable = match handle {
                Some(handle) => {
                    matches!(handle.is_read_only(), Ok(false))
                        && matches!(handle.owns_editor(context.view_id), Ok(true))
                        && matches!(handle.revision(), Ok(revision) if revision == session.revision)
                        && handle.edits(context.view_id, &session.id)
                }
                None => false,
            };
            if !exists || !writable {
                let mut editor = context.editor;
                editor.set(None);
                if let Some(handle) = handle {
                    if handle.edits(context.view_id, &session.id) {
                        context.report(handle.cancel_pending());
                    }
                }
            }
        }
    });
    use_effect(move || {
        if let Some(handle) = handle {
            let revision = handle.revision();
            let node_enabled = handle.has_plugin(Capability::DragDrop)
                && matches!(handle.is_read_only(), Ok(false));
            let select_enabled = handle.has_plugin(Capability::MultiSelect);
            let stale = gesture.peek().as_ref().is_some_and(|current| {
                revision != Ok(current.revision)
                    || if current.source.is_some() {
                        !node_enabled
                    } else {
                        !select_enabled
                    }
            });
            if stale {
                gesture.set(None);
            }
            if !handle.has_plugin(Capability::ContextMenu) {
                menu.set(None);
            }
        }
    });
    let selection = match handle {
        Some(handle) => handle.selection(),
        None => Ok(Vec::new()),
    };
    let selection = match selection {
        Ok(selection) => selection,
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let paths = match &*geometry.read() {
        Some(Ok(paths)) => paths.clone(),
        _ => Vec::new(),
    };
    let direction = match document.direction {
        LayoutDirection::Left => "left",
        LayoutDirection::Right => "right",
        LayoutDirection::Side => "side",
        LayoutDirection::Down => "down",
    };
    let align = if alignment == Alignment::Nodes {
        "nodes"
    } else {
        "root"
    };
    let style = format!(
        "{}{style}",
        theme.as_ref().map(Theme::css).unwrap_or_default()
    );
    let root = &document.node_data;
    let validation = document.validate();
    rsx! {
        style { "{STYLESHEET}" }
        div {
            class: "dme-map {class}", style,
            role: "group", aria_label: "Mind map", tabindex: "0",
            "data-layout": direction, "data-alignment": align, "data-compact": document.compact,
            onmounted: move |event| container.set(Some(event.data())),
            onwheel: move |event| {
                event.prevent_default();
                let delta = event.delta().strip_units();
                let result = if event.modifiers().ctrl() || event.modifiers().meta() {
                    // Wheel coordinates are measured against the view container,
                    // not the possibly nested event target.
                    let at = event.client_coordinates();
                    let element = container.read().clone();
                    spawn(async move {
                        if let Some(element) = element {
                            let result = async {
                                let rect = platform::measure(&element).await?;
                                let mut next = *viewport.peek();
                                next.zoom_at(next.scale * (-delta.y * 0.002).exp(), Point { x: at.x - rect.x, y: at.y - rect.y })?;
                                viewport.set(next); Ok(())
                            }.await;
                            context.report(result);
                        }
                    });
                    Ok(())
                } else {
                    let mut next = *viewport.peek();
                    let result = next.pan(Point { x: -delta.x, y: -delta.y });
                    if result.is_ok() { viewport.set(next); } result
                };
                context.report(result);
            },
            onpointerdown: move |event| {
                if event.trigger_button() == Some(MouseButton::Primary) {
                    let at = event.client_coordinates(); let point = Point { x: at.x, y: at.y };
                    menu.set(None);
                    if event.modifiers().shift() && handle.is_some_and(|h| h.has_plugin(Capability::MultiSelect)) {
                        {let next=gesture_epoch.peek().wrapping_add(1);gesture_epoch.set(next);}
                        if let Some(handle)=handle {
                            match handle.select(&[]).and_then(|_|handle.revision()) {
                                Ok(revision)=>gesture.set(Some(NodeDrag {start: point,current:point,source:None,active:false,revision})),
                                Err(error)=>context.on_error.call(error),
                            }
                        }
                    } else { drag.set(Some(point)); }
                    if let Some(handle) = handle { context.report(handle.select(&[])); }
                }
            },
            onpointermove: move |event| {
                let at = event.client_coordinates(); let point = Point { x: at.x,y:at.y };
                let current_gesture = gesture.peek().clone();
                if let Some(mut current) = current_gesture {
                    current.current = point; current.active = current.active || gesture::is_drag(current.start,point);
                    gesture.set(Some(current));
                    return;
                }
                let previous = *drag.peek();
                if let Some(previous) = previous {
                    let at = event.client_coordinates(); let point = Point { x: at.x, y: at.y };
                    let mut next = *viewport.peek();
                    let result = next.pan(Point { x: point.x - previous.x, y: point.y - previous.y });
                    if result.is_ok() { viewport.set(next); drag.set(Some(point)); } context.report(result);
                }
            },
            onpointerup: move |_| {
                drag.set(None); let current = gesture.peek().clone(); gesture.set(None);
                if let (Some(current),Some(handle)) = (current,handle) {
                    if current.active {
                        let mut suppress_click = suppress_click; suppress_click.set(true);
                        let epoch=*gesture_epoch.peek();
                        let ids = handle.selection();
                        let elements = mounts.peek().clone(); let revision = handle.revision();
                        spawn(async move {
                            let result = async {
                                let revision = revision?; let data = handle.document()?; let mut selected = Vec::new(); let mut placement = None;
                                for (id,mounted) in elements {
                                    let r = platform::measure(&mounted).await?;
                                    let rect = HitRect {x:r.x,y:r.y,width:r.width,height:r.height};
                                    if current.source.is_some() {
                                        if rect.contains(current.current) && current.source.as_ref() != Some(&id) {
                                            placement = Some(gesture::drop_placement(&id,rect,current.current,data.direction,viewport.peek().scale)?);
                                        }
                                    } else if rect.intersects(current.start,current.current) { selected.push(id); }
                                }
                                if handle.revision()? != revision || revision != current.revision || *gesture_epoch.peek()!=epoch { return Err(MindMapError::StaleOperation); }
                                if current.source.is_some() {
                                    if !handle.has_plugin(Capability::DragDrop) { return Err(MindMapError::StaleOperation); }
                                    if let Some(placement) = placement { handle.apply(Operation::Move {ids:ids?,placement})?; }
                                } else {
                                    if !handle.has_plugin(Capability::MultiSelect) { return Err(MindMapError::StaleOperation); }
                                    handle.select(&selected)?;
                                }
                                Ok(())
                            }.await; context.report(result);
                        });
                    }
                }
            },
            onpointercancel: move |_| { drag.set(None); gesture.set(None); {let next=gesture_epoch.peek().wrapping_add(1);gesture_epoch.set(next);} },
            onpointerleave: move |_| { drag.set(None); gesture.set(None); {let next=gesture_epoch.peek().wrapping_add(1);gesture_epoch.set(next);} },
            onkeydown: move |event| {
                if event.key() == Key::Escape { gesture.set(None); menu.set(None); {let next=gesture_epoch.peek().wrapping_add(1);gesture_epoch.set(next);} }
                if event.key() == Key::Home && !event.is_composing() { event.prevent_default(); viewport.set(Viewport::default()); }
            },
            if let Err(error) = validation { p { class: "dme-error", role: "alert", "{error}" } }
            else {
                CanvasTransform { viewport, canvas, scale_probe,

                    svg { class: "dme-links", "aria-hidden": "true",
                        for (color, path) in paths {
                            path { class: "dme-link", d: path, style: color.map(|color| format!("stroke:{color}")) }
                        }
                    }
                    if document.direction == LayoutDirection::Down {
                        div { class: "dme-root", TopicView { key: "{root.id}", node: root.clone(), root: true, selected: selection.contains(&root.id) } }
                        if root.expanded {
                            div { class: "dme-main dme-down",
                                for child in &root.children { NodeView { key: "{child.id}", node: child.clone(), direction: LayoutDirection::Down, selection: selection.clone() } }
                            }
                        }
                    } else {
                        div { class: "dme-main dme-left",
                            if root.expanded {
                                for child in root.children.iter().filter(|node| side(document.direction, &root.children, &node.id) == LayoutDirection::Left) {
                                    NodeView { key: "{child.id}", node: child.clone(), direction: LayoutDirection::Left, selection: selection.clone() }
                                }
                            }
                        }
                        div { class: "dme-root", TopicView { key: "{root.id}", node: root.clone(), root: true, selected: selection.contains(&root.id) } }
                        div { class: "dme-main dme-right",
                            if root.expanded {
                                for child in root.children.iter().filter(|node| side(document.direction, &root.children, &node.id) == LayoutDirection::Right) {
                                    NodeView { key: "{child.id}", node: child.clone(), direction: LayoutDirection::Right, selection: selection.clone() }
                                }
                            }
                        }
                    }
                }
            }
            GestureOverlay { gesture, locale, mounts, viewport, container, handle }
            if let (Some(at),Some(handle)) = (menu(),handle) {
                div { class: "dme-context-menu", role: "menu", style: format!("position:fixed;left:clamp(8px,{}px,calc(100vw - var(--dme-menu-width,200px) - 8px));top:clamp(8px,{}px,40vh)",at.x,at.y),
                    onpointerdown: move |event| event.stop_propagation(),
                    match handle.plugin_commands() {
                        Ok(commands) => rsx! { for command in commands {
                            button { r#type: "button", role: "menuitem", onclick: move |_| { menu.set(None); context.report(plugins::execute(handle,command,locale)); }, "{locale.label(command)}" }
                        } },
                        Err(error) => rsx! { p {role: "alert", "{error}"} },
                    }
                }
            }
            if let Some(error) = measurement_error() { p { class: "dme-error", role: "alert", "{error}" } }
        }
    }
}

// Isolate high-frequency viewport updates: pan only updates this transform,
// without rerunning MapBody or cloning the document/tree component props.
#[component]
fn CanvasTransform(
    viewport: Signal<Viewport>,
    mut canvas: Signal<Option<Rc<MountedData>>>,
    mut scale_probe: Signal<Option<Rc<MountedData>>>,
    children: Element,
) -> Element {
    let context = use_context::<ViewContext>();
    let view = viewport();
    if let Err(error) = view.validate() {
        return rsx! { p { class: "dme-error", role: "alert", "{error}" } };
    }
    let transform = format!(
        "transform:translate({}px,{}px) scale({});",
        view.translation.x, view.translation.y, view.scale
    );
    rsx! { div { class: "dme-canvas", style: transform, "data-scale": "{view.scale}",
        onmounted: move |event| canvas.set(Some(event.data())),
        onresize: move |_| context.resized(),
        div { class: "dme-scale-probe", aria_hidden: "true", onmounted: move |event| scale_probe.set(Some(event.data())) }
        {children}
    } }
}

fn assigned_side(children: &[Node], id: &str) -> LayoutDirection {
    let (mut left, mut right) = (0usize, 0usize);
    for child in children {
        let direction = match child.direction {
            Some(NodeDirection::Left) => LayoutDirection::Left,
            Some(NodeDirection::Right) => LayoutDirection::Right,
            None if left <= right => LayoutDirection::Left,
            None => LayoutDirection::Right,
        };
        if child.id == id {
            return direction;
        }
        if direction == LayoutDirection::Left {
            left += 1;
        } else {
            right += 1;
        }
    }
    LayoutDirection::Right
}
fn side(direction: LayoutDirection, children: &[Node], id: &str) -> LayoutDirection {
    if direction == LayoutDirection::Side {
        assigned_side(children, id)
    } else {
        direction
    }
}

#[component]
fn NodeView(node: Node, direction: LayoutDirection, selection: Vec<String>) -> Element {
    let context = use_context::<ViewContext>();
    let class = match direction {
        LayoutDirection::Left => "dme-branch dme-left",
        LayoutDirection::Down => "dme-branch dme-down",
        _ => "dme-branch dme-right",
    };
    rsx! {
        div { class, onresize: move |_| context.resized(),
            TopicView { key: "{node.id}", node: node.clone(), root: false, direction, selected: selection.contains(&node.id) }
            if node.expanded && !node.children.is_empty() {
                div { class: "dme-children", onresize: move |_| context.resized(),
                    for child in node.children { NodeView { key: "{child.id}", node: child, direction, selection: selection.clone() } }
                }
            }
        }
    }
}

#[component]
fn TopicView(
    node: Node,
    root: bool,
    #[props(default)] direction: LayoutDirection,
    selected: bool,
) -> Element {
    let mut context = use_context::<ViewContext>();
    let mut mounts = context.mounts;
    let mut own_element = use_signal(|| None::<Rc<MountedData>>);
    let cleanup_id = node.id.clone();
    use_drop(move || {
        if let Ok(mut mounts) = mounts.try_write() {
            mounts.remove(&cleanup_id);
        }
    });
    let mut focused = use_signal(|| false);
    let mount_id = node.id.clone();
    let click_id = node.id.clone();
    let drag_id = node.id.clone();
    let context_id = node.id.clone();
    let edit_node = node.clone();
    let keyboard_node = node.clone();
    let collapse_id = node.id.clone();
    let expanded = node.expanded;
    let editing = context
        .editor
        .read()
        .as_ref()
        .is_some_and(|session| session.id == node.id);
    let style = node.style.clone().unwrap_or_default();
    let image = node.image.clone().map(|image| {
        let src = match context.image_url {
            Some(resolve) => resolve.call(image.url.clone()),
            None => image.url.clone(),
        };
        (src, image)
    });
    let mut broken = use_signal(|| None::<String>);
    let has_note = node.note.is_some();
    let has_image = image.is_some();
    let image_width = image
        .as_ref()
        .map(|(_, image)| format!("--dme-image-width:{}px", image.width));
    let flow = match direction {
        LayoutDirection::Left => "left",
        LayoutDirection::Down => "down",
        _ => "right",
    };
    rsx! {
        div { key: "{node.id}-{editing}", class: "dme-topic", autofocus: selected && !editing, "data-node-id": "{node.id}", "data-root": root, "data-selected": selected, "data-flow": flow,
            tabindex: "0", role: "treeitem", aria_selected: selected,
            aria_expanded: if node.children.is_empty() { None } else { Some(expanded.to_string()) },
            font_size: style.font_size, font_family: style.font_family, color: style.color, background: style.background,
            font_weight: style.font_weight, width: style.width, border: style.border, text_decoration: style.text_decoration,
            "data-image": has_image, "data-note": has_note, style: image_width,
            onmounted: move |event| {
                own_element.set(Some(event.data()));
                mounts.write().insert(mount_id.clone(), event.data());
                if selected && !editing {
                    spawn(async move {
                        if let Err(error) = event.data().set_focus(true).await {
                            if !*focused.peek() { context.on_error.call(MindMapError::Measurement(error.to_string())); }
                        }
                    });
                }
            },
            onfocus: move |_| focused.set(true),
            onblur: move |_| focused.set(false),
            onresize: move |_| context.resized(),
            onpointerdown: move |event| {
                event.stop_propagation();
                context.suppress_click.set(false);
                if event.trigger_button() == Some(MouseButton::Primary) && !event.modifiers().shift() && !event.modifiers().ctrl() && !event.modifiers().meta() {
                    if let Some(handle) = context.handle {
                        if handle.has_plugin(Capability::DragDrop) && matches!(handle.is_read_only(),Ok(false)) && !root && !editing {
                            if !handle.selection().is_ok_and(|ids| ids.contains(&drag_id)) { context.select(&drag_id); }
                            let at=event.client_coordinates(); let at=Point{x:at.x,y:at.y};
                            let next=context.gesture_epoch.peek().wrapping_add(1);context.gesture_epoch.set(next);
                            match handle.revision() {
                                Ok(revision)=>context.gesture.set(Some(NodeDrag {start:at,current:at,source:Some(drag_id.clone()),active:false,revision})),
                                Err(error)=>context.on_error.call(error),
                            }
                        }
                    }
                }
            },
            oncontextmenu: move |event| {
                if context.handle.is_some_and(|h| h.has_plugin(Capability::ContextMenu)) {
                    event.prevent_default(); event.stop_propagation(); context.select(&context_id);
                    let at=event.client_coordinates(); context.menu.set(Some(Point{x:at.x,y:at.y}));
                }
            },
            onclick: move |event| { event.stop_propagation(); if *context.suppress_click.peek() {context.suppress_click.set(false); return;} context.click_select(&click_id,event.modifiers().ctrl() || event.modifiers().meta() || event.modifiers().shift()); },
            ondoubleclick: move |event| { event.stop_propagation(); context.start_edit(&edit_node); },
            onkeydown: move |event| {
                if event.is_composing() { return; }
                if event.key() == Key::F2 { event.prevent_default(); event.stop_propagation(); context.start_edit(&keyboard_node); } else { context.keyboard(event, &keyboard_node); }
            },
            if let Some((src, image)) = image {
                if broken.read().as_deref() == Some(src.as_str()) {
                    span { class: "dme-image-missing", role: "img", aria_label: image.alt.clone().unwrap_or_default(),
                        style: format!("width:{}px;height:{}px", image.width, image.height),
                        if context.locale == Locale::Zh { "图片无法显示" } else { "Image unavailable" }
                    }
                } else {
                    img { class: "dme-image", src: "{src}", alt: image.alt.clone().unwrap_or_default(), draggable: "false",
                        width: "{image.width}", height: "{image.height}",
                        style: format!("width:{}px;height:{}px", image.width, image.height),
                        onerror: move |_| broken.set(Some(src.clone())),
                        onload: move |_| context.resized(),
                    }
                }
            }
            if editing {
                if let Some(session) = context.editor.read().clone() {
                    TopicEditor { key: "{session.id}-{session.revision}", session }
                }
            } else { span { class: "dme-topic-text", "{node.topic}" } }
            if has_note && !editing {
                span { class: "dme-note-marker", aria_label: if context.locale == Locale::Zh { "有备注" } else { "Has note" }, title: if context.locale == Locale::Zh { "有备注" } else { "Has note" } }
            }
            if selected && !editing {
                span { class: "dme-focus-marker", aria_hidden: "true",
                    onmounted: move |_| async move {
                        if let Some(handle) = context.handle {
                            if matches!(handle.owns_editor(context.view_id), Ok(true)) {
                                let element = own_element.peek().clone();
                                if let Some(element) = element {
                                    if let Err(error) = element.set_focus(true).await {
                                        if !*focused.peek() { context.on_error.call(MindMapError::Measurement(error.to_string())); }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !root && !editing && !node.children.is_empty() && context.handle.is_some() {
                button { class: "dme-collapse", r#type: "button", "data-expanded": expanded, aria_expanded: expanded, aria_label: match (context.locale,expanded) {(Locale::Zh,true)=>"折叠",(Locale::Zh,false)=>"展开",(_,true)=>"Collapse",(_,false)=>"Expand"},
                    onpointerdown: move |event| event.stop_propagation(),
                    onclick: move |event| {
                        event.stop_propagation();
                        if let Some(handle) = context.handle { context.report(handle.apply(Operation::SetExpanded { id: collapse_id.clone(), expanded: !expanded }).map(|_| ())); }
                    },
                    ondoubleclick: move |event| event.stop_propagation(),
                    onkeydown: move |event| event.stop_propagation(),
                    span { class: "dme-collapse-icon", aria_hidden: "true" }
                }
            }
        }
    }
}

#[component]
fn TopicEditor(session: EditSession) -> Element {
    let context = use_context::<ViewContext>();
    let mut draft = use_signal(|| session.original.clone());
    let mut composing = use_signal(|| false);
    let mut composition_ended = use_signal(|| false);
    let mut finished = use_signal(|| false);
    let mut focused = use_signal(|| false);
    let mut editor = context.editor;
    let commit = move || -> Result<(), MindMapError> {
        let handle = context.handle.ok_or(MindMapError::StaleOperation)?;
        if handle.revision()? != session.revision {
            return Err(MindMapError::StaleOperation);
        }
        handle.update_draft(context.view_id, draft.peek().clone());
        handle.commit_pending().map(|_| ())
    };
    let discard = move || {
        if let Some(handle) = context.handle {
            context.report(handle.cancel_pending());
        }
    };
    rsx! {
        textarea { class: "dme-editor", autofocus: true, aria_label: "Edit node", rows: "3", value: session.original,
            onmounted: move |event| async move {
                if let Err(error) = event.data().set_focus(true).await {
                    // Dioxus Desktop 0.7.10 returns null for successful focus.
                    // Only a received native focus event confirms success.
                    if !*focused.peek() { context.on_error.call(MindMapError::Measurement(error.to_string())); }
                }
            },
            onfocus: move |_| focused.set(true),
            oninput: move |event| {
                let value = event.value();
                if let Some(handle) = context.handle { handle.update_draft(context.view_id, value.clone()); }
                draft.set(value); context.resized();
            },
            oncompositionstart: move |_| { composing.set(true); composition_ended.set(false); },
            oncompositionend: move |_| { composing.set(false); composition_ended.set(true); },
            onkeyup: move |_| composition_ended.set(false),
            onclick: move |event| event.stop_propagation(),
            onwheel: move |event| event.stop_propagation(),
            onkeydown: move |event| {
                event.stop_propagation();
                if composing() || composition_ended() || event.is_composing() { return; }
                let cancel = event.key() == Key::Escape;
                let submit = event.key() == Key::Tab || (event.key() == Key::Enter && !event.modifiers().shift());
                if cancel || submit {
                    event.prevent_default(); finished.set(true);
                    if submit { context.report(commit()); } else { discard(); }
                    editor.set(None);

                }
            },
            onblur: move |_| {
                // Blur cancels by default; hosts that protect drafts opt into a
                // commit. Composition never blurs, so IME confirmation is safe.
                if !finished() {
                    finished.set(true);
                    if context.blur_commits { context.report(commit()); } else { discard(); }
                    editor.set(None);
                }
            },
        }
    }
}

#[component]
fn GestureOverlay(
    gesture: Signal<Option<NodeDrag>>,
    locale: Locale,
    mounts: Signal<Mounts>,
    mut viewport: Signal<Viewport>,
    container: Signal<Option<Rc<MountedData>>>,
    handle: Option<MindMapHandle>,
) -> Element {
    let edge = use_resource(move || async move {
        let current = gesture.read().clone();
        let enabled = handle.is_some_and(|handle| handle.has_plugin(Capability::EdgePan));
        let Some(current) =
            current.filter(|current| current.active && current.source.is_some() && enabled)
        else {
            return Ok::<(), MindMapError>(());
        };
        let mounted = container.peek().clone();
        let Some(mounted) = mounted else {
            return Ok::<(), MindMapError>(());
        };
        loop {
            let rect = platform::measure(&mounted).await?;
            let delta = gesture::edge_pan(
                HitRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                },
                current.current,
            )?;
            if delta == Point::default() {
                return Ok::<(), MindMapError>(());
            }
            // The resource is cancelled on pointer movement, release, disable or unmount.
            if gesture.peek().as_ref() != Some(&current) {
                return Ok::<(), MindMapError>(());
            }
            let mut next = *viewport.peek();
            next.pan(delta)?;
            viewport.set(next);
            futures_timer::Delay::new(std::time::Duration::from_millis(16)).await;
        }
    });
    let target = use_resource(move || async move {
        let current = gesture.read().clone();
        let elements = mounts.peek().clone();
        let Some(current) = current.filter(|current| current.active && current.source.is_some())
        else {
            return Ok(None);
        };
        for (id, mounted) in elements {
            if current.source.as_ref() == Some(&id) {
                continue;
            }
            let r = platform::measure(&mounted).await?;
            let rect = HitRect {
                x: r.x,
                y: r.y,
                width: r.width,
                height: r.height,
            };
            if rect.contains(current.current) {
                return Ok::<_, MindMapError>(Some((id, rect, current.current)));
            }
        }
        Ok(None)
    });
    rsx! {
        if let Some(Err(error))=&*edge.read() {span {class:"dme-error",role:"alert","{error}"}}
        if let Some(current)=gesture() {
            if current.active {
                if current.source.is_some() {
                    div {class:"dme-drag-status",role:"status", if locale==Locale::Zh {"拖动节点 · Esc 取消"} else {"Moving branches · Esc to cancel"}}
                    if let Some(Ok(Some((id,rect,at))))=&*target.read() {
                        div {class:"dme-drop-target", "data-drop-id":id.clone(), style:format!("position:fixed;left:{}px;top:{}px;width:{}px;height:{}px;--dme-drop-band:{}px;",rect.x,rect.y,rect.width,rect.height,12.0*viewport.peek().scale),
                            span {class:"dme-drop-label", if locale==Locale::Zh {"释放以移动"} else {"Drop to move"}}
                        }
                        // The hit test used by commit decides before / after / child.
                        span {class:"dme-drag-point",style:format!("position:fixed;left:{}px;top:{}px",at.x,at.y)}
                    }
                    if let Some(Err(error))=&*target.read() {span {class:"dme-error",role:"alert","{error}"}}
                } else {
                    div {class:"dme-marquee",style:format!("position:fixed;left:{}px;top:{}px;width:{}px;height:{}px",current.start.x.min(current.current.x),current.start.y.min(current.current.y),(current.current.x-current.start.x).abs(),(current.current.y-current.start.y).abs())}
                }
            }
        }
    }
}
