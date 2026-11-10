//! Public bilingual playground. All editor behavior comes from the library.
use std::rc::Rc;

use dioxus::prelude::*;
use dioxus_mind_elixir::{
    EditableMindMap, LayoutDirection, MindMapData, MindMapError, MindMapHandle, Node, Operation,
    Outline, Toolbar, Viewport,
    plugins::{
        Locale, Plugin,
        export::{self, ExportFile, ImageOptions},
    },
    use_mind_map, use_plugins,
};

fn main() {
    dioxus::launch(App);
}

fn label(locale: Locale, en: &'static str, zh: &'static str) -> &'static str {
    match locale {
        Locale::En => en,
        Locale::Zh => zh,
    }
}

fn initial_locale() -> Locale {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    if let Some(window) = web_sys::window() {
        if let Ok(search) = window.location().search() {
            if search
                .trim_start_matches('?')
                .split('&')
                .any(|part| part == "lang=zh")
            {
                return Locale::Zh;
            }
        }
    }
    Locale::En
}

fn sample(locale: Locale) -> MindMapData {
    let mut data = MindMapData::new(
        Node::new("root", label(locale, "Project notes", "项目笔记")).with_children(vec![
            Node::new("explore", label(locale, "Explore", "探索")).with_children(vec![
                Node::new(
                    "question",
                    label(locale, "Start with a question", "从一个问题开始"),
                ),
                Node::new(
                    "connect",
                    label(locale, "Connect the dots", "连接零散的灵感"),
                ),
            ]),
            Node::new("make", label(locale, "Customize", "样式定制")).with_children(vec![
                Node::new(
                    "edit",
                    label(locale, "Double-click to edit", "双击编辑节点"),
                ),
                Node::new(
                    "theme",
                    label(locale, "Style with CSS variables", "用 CSS 变量改变样式"),
                ),
            ]),
            Node::new("share", label(locale, "Export", "导出")).with_children(vec![
                Node::new(
                    "json",
                    label(locale, "Save the full tree as JSON", "用 JSON 保存完整脑图"),
                ),
                Node::new(
                    "export",
                    label(locale, "Export an image or XMind", "导出图片或 XMind"),
                ),
            ]),
        ]),
    );
    data.direction = LayoutDirection::Side;
    data
}

#[component]
fn App() -> Element {
    let mut locale = use_signal(initial_locale);
    let handle = match use_mind_map(sample(locale())) {
        Ok(handle) => handle,
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let _plugins = match use_plugins(handle, Plugin::builtins()) {
        Ok(plugins) => plugins,
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let mut error = use_signal(String::new);
    let mut status = use_signal(String::new);
    let mut dark = use_signal(|| false);
    let mut outline = use_signal(|| true);
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    let mut viewport = use_signal(Viewport::default);
    let mut view_width = use_signal(|| 0.0_f64);
    let mut center = use_signal(|| 1u64);
    let mut busy = use_signal(|| false);
    let mut json_input = use_signal(String::new);
    let mut reset_pending = use_signal(|| false);
    let on_error = use_callback(move |value: MindMapError| error.set(value.to_string()));
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    use_effect(move || {
        let lang = label(locale(), "en", "zh-CN");
        if let Some(root) = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.document_element())
        {
            if let Err(value) = root.set_attribute("lang", lang) {
                error.set(format!("Cannot set document language: {value:?}"));
            }
        }
    });
    let lang = locale();
    let data = match handle.document() {
        Ok(data) => data,
        Err(value) => return rsx! { p { role: "alert", "{value}" } },
    };
    let layout = data.direction;
    let read_only = matches!(handle.is_read_only(), Ok(true));
    let node_count = count_nodes(&data.node_data);
    rsx! {
        style { {include_str!("../playground.css")} }
        div { class: if dark() { "site dark" } else { "site" }, lang: label(lang, "en", "zh-CN"),
            nav { class: "topbar", aria_label: label(lang, "Main navigation", "主导航"),
                a { class: "wordmark", href: "#", "dioxus-mind-elixir" }
                div { class: "nav-links",
                    a { href: "https://docs.rs/dioxus-mind-elixir/0.1.0/dioxus_mind_elixir/", target: "_blank", rel: "noopener noreferrer", {label(lang, "API docs ↗", "API 文档 ↗")} }
                    a { href: "https://github.com/pk111and222/dioxus-mind-elixir", target: "_blank", rel: "noopener noreferrer", "GitHub ↗" }
                    button { id: "language", class: "language", onclick: move |_| {
                        locale.set(if locale() == Locale::En { Locale::Zh } else { Locale::En });
                        status.set(String::new());
                        reset_pending.set(false);
                    }, {label(lang, "中文", "English")} }
                }
            }
            main {
                header { class: "hero",
                    div {
                        p { class: "eyebrow", "dioxus-mind-elixir / 0.1.0" }
                        h1 { {label(lang, "Mind maps for Dioxus", "Dioxus 脑图组件")} }
                        p { class: "intro", {label(lang, "A Rust component for editing mind maps, with a shared outline, CSS variables and file exports. Try it below.", "用 Rust 编写的可编辑脑图组件，支持共享大纲、CSS 变量和文件导出。下方可以直接试用。")} }
                    }
                    aside { class: "origin",

                        h2 { {label(lang, "Forked from Mind Elixir", "Fork 自 Mind Elixir")} }
                        p { {label(lang, "This is the Rust + Dioxus component version of Mind Elixir Core, rewritten in Rust from its data and interaction design.", "这是 Mind Elixir Core 的 Rust + Dioxus 组件版本，基于上游的数据与交互设计，以 Rust 重新实现。")} }
                        div { class: "origin-links",
                            a { href: "https://mind-elixir.com/", target: "_blank", rel: "noopener noreferrer", {label(lang, "Official website ↗", "上游官网 ↗")} }
                            a { href: "https://github.com/ssshooter/mind-elixir-core", target: "_blank", rel: "noopener noreferrer", {label(lang, "Upstream GitHub ↗", "上游 GitHub ↗")} }
                        }
                    }
                }
                section { class: "playground", aria_label: label(lang, "Interactive playground", "交互演示"),
                    div { class: "panel-heading",
                        div { class: "panel-title", h2 { {label(lang, "Demo", "在线演示")} } span { class: "version", "v0.1.0" } }
                        div { class: "view-controls",
                            button { id: "theme", aria_pressed: dark(), onclick: move |_| dark.set(!dark()), {label(lang, "Light / dark", "明 / 暗主题")} }
                            button { id: "outline-toggle", aria_pressed: outline(), onclick: move |_| outline.set(!outline()), {label(lang, "Outline", "大纲")} }
                            button { id: "readonly", aria_pressed: read_only, onclick: move |_| {
                                if let Err(value) = handle.set_read_only(!read_only) { on_error.call(value); }
                            }, {label(lang, "Read only", "只读")} }
                        }
                    }
                    div { class: "layout-bar",
                        div { class: "segmented", role: "group", aria_label: label(lang, "Layout", "布局"),
                            for (direction, en, zh) in [
                                (LayoutDirection::Side, "Both sides", "左右"),
                                (LayoutDirection::Right, "Right", "向右"),
                                (LayoutDirection::Left, "Left", "向左"),
                                (LayoutDirection::Down, "Down", "向下"),
                            ] {
                                button { class: if layout == direction { "active" } else { "" }, aria_pressed: layout == direction, disabled: read_only,
                                    onclick: move |_| {
                                        match handle.apply(Operation::SetLayout(direction)) {
                                            Ok(_) => center.set(center().saturating_add(1)),
                                            Err(value) => on_error.call(value),
                                        }
                                    }, {label(lang, en, zh)}
                                }
                            }
                        }
                        div { class: "layout-actions",
                            button { id: "zoom-out", aria_label: label(lang, "Zoom out", "缩小"), onclick: move |_| {
                                let next = (viewport().scale / 1.2).clamp(0.1, 4.0);
                                viewport.write().scale = next;
                                center.set(center().saturating_add(1));
                            }, "−" }
                            button { id: "zoom-in", aria_label: label(lang, "Zoom in", "放大"), onclick: move |_| {
                                let next = (viewport().scale * 1.2).clamp(0.1, 4.0);
                                viewport.write().scale = next;
                                center.set(center().saturating_add(1));
                            }, "+" }
                            button { id: "center", onclick: move |_| center.set(center().saturating_add(1)), {label(lang, "Center view", "居中")} }
                            button { id: "reset", onclick: move |_| reset_pending.set(!reset_pending()), {label(lang, "Reset sample", "重置示例")} }
                        }
                    }
                    if reset_pending() {
                        div { class: "reset-confirm", role: "group", aria_label: label(lang, "Confirm reset", "确认重置"),
                            p { {label(lang, "Replace your edits and history with a fresh sample? Download JSON first to keep your work.", "将用新示例替换当前编辑和历史；如需保留，请先下载 JSON。")} }
                            button { onclick: move |_| {
                                match handle.set_read_only(false).and_then(|()| handle.refresh(sample(lang))) {
                                    Ok(()) => {
                                        error.set(String::new()); status.set(String::new()); reset_pending.set(false);
                                    }
                                    Err(value) => on_error.call(value),
                                }
                            }, {label(lang, "Replace with sample", "确认替换")} }
                            button { onclick: move |_| reset_pending.set(false), {label(lang, "Cancel", "取消")} }
                        }
                    }
                    Toolbar { handle, locale: lang, on_error }
                    div { class: if outline() { "editor-grid" } else { "editor-grid no-outline" },
                        div { class: "map-host", onmounted: move |event| mounted.set(Some(event.data())),
                            onresize: move |event| {
                                match event.get_content_box_size() {
                                    Ok(size) if size.width > 0.0 && (size.width - view_width()).abs() > 1.0 => {
                                        view_width.set(size.width);
                                        // Start the public sample at a useful scale on narrow screens.
                                        // Users retain explicit zoom controls after resizing.
                                        viewport.write().scale = (size.width / 800.0).clamp(0.45, 1.0);
                                        center.set(center().saturating_add(1));
                                    }
                                    Ok(_) => {}
                                    Err(value) => on_error.call(MindMapError::Measurement(value.to_string())),
                                }
                            },
                            EditableMindMap { handle, locale: lang, on_error, viewport, center_revision: center() }
                        }
                        if outline() {
                            aside { class: "outline-host", aria_label: label(lang, "Shared outline", "共享大纲"),
                                p { class: "small-label", {label(lang, "Outline", "大纲")} }
                                Outline { handle, locale: lang, on_error }
                            }
                        }
                    }
                    div { class: "canvas-footer",
                        span { "{node_count} " {label(lang, "nodes · edits stay in this tab", "个节点 · 编辑仅保存在当前标签页")} }
                        span { {label(lang, "Double-click to edit · drag blank space to pan", "双击编辑 · 拖动画布空白处平移")} }
                    }
                }
                div { class: "below-grid",
                    section { class: "card export-card",

                        h2 { {label(lang, "Export", "导出")} }
                        p { {label(lang, "JSON preserves the full document. XMind keeps the supported tree fields; SVG, PNG and HTML capture the live Web appearance.", "JSON 保存完整文档；XMind 导出支持的树字段。SVG、PNG 和 HTML 则保留当前 Web 页面中的实际样式。")} }
                        div { class: "export-buttons",
                            for format in ["JSON", "XMind", "SVG", "PNG", "HTML"] {
                                button { id: "export-{format}", disabled: busy(), onclick: move |_| {
                                    let element = mounted.peek().clone();
                                    error.set(String::new());
                                    status.set(label(lang, "Preparing download…", "正在准备下载…").into());
                                    busy.set(true);
                                    spawn(async move {
                                        let result = prepare_export(handle, element, format).await.and_then(download);
                                        busy.set(false);
                                        match result {
                                            Ok(()) => status.set(format!("{} {format}", label(lang, "Download started:", "已开始下载："))),
                                            Err(value) => { status.set(String::new()); on_error.call(value); }
                                        }
                                    });
                                }, "{format} ↓" }
                            }
                        }
                        p { class: "status", role: "status", aria_live: "polite", "{status}" }
                        details {
                            summary { {label(lang, "Load or inspect JSON", "载入或查看 JSON")} }
                            p { {label(lang, "Paste a supported document. Loading replaces the current document and history. Nothing is sent to a server.", "粘贴支持格式的文档；载入会替换当前文档与历史。数据不会上传服务器。")} }
                            textarea { id: "json-input", aria_label: label(lang, "Mind map JSON", "脑图 JSON"), value: "{json_input}", oninput: move |event| json_input.set(event.value()) }
                            div { class: "export-buttons",
                                button { disabled: read_only || busy(), onclick: move |_| {
                                    match MindMapData::from_json(&json_input()).and_then(|data| handle.refresh(data)) {
                                        Ok(()) => { error.set(String::new()); status.set(label(lang, "JSON loaded", "JSON 已载入").into()); }
                                        Err(value) => on_error.call(value),
                                    }
                                }, {label(lang, "Load JSON", "载入 JSON")} }
                                button { onclick: move |_| {
                                    match handle.document().and_then(|data| data.to_json()) {
                                        Ok(value) => json_input.set(value),
                                        Err(value) => on_error.call(value),
                                    }
                                }, {label(lang, "Show current JSON", "查看当前 JSON")} }
                            }
                        }
                    }
                    section { class: "card",

                        h2 { {label(lang, "Keyboard shortcuts", "快捷键")} }
                        dl { class: "shortcuts",
                            div { dt { kbd { "Tab" } } dd { {label(lang, "Add a child", "添加子节点")} } }
                            div { dt { kbd { "Enter" } } dd { {label(lang, "Add a sibling", "添加同级节点")} } }
                            div { dt { kbd { "F2" } } dd { {label(lang, "Edit the selected node", "编辑选中节点")} } }
                            div { dt { kbd { "Ctrl / ⌘ Z" } } dd { {label(lang, "Undo your last change", "撤销上次修改")} } }
                        }
                        p { class: "quiet", {label(lang, "Ctrl/⌘ + click selects multiple nodes. Shift + drag on blank space selects a region. Language changes the interface, not your document; reset for a localized sample.", "Ctrl/⌘ + 点击多选；Shift + 拖动空白处框选。切换语言仅改变界面，不翻译文档；重置可载入对应语言的示例。")} }
                    }
                }
                if !error().is_empty() {
                    div { class: "error", role: "alert", p { "{error}" } button { onclick: move |_| error.set(String::new()), {label(lang, "Dismiss", "关闭")} } }
                }
                section { class: "install-strip",
                    div { h2 { {label(lang, "Use in your project", "在项目中使用")} } p { "Rust 1.85+ · Dioxus 0.7.10 · dioxus-mind-elixir 0.1.0" } }
                    div { code { "cargo add dioxus-mind-elixir@0.1.0 --features web" } a { href: label(lang, "https://github.com/pk111and222/dioxus-mind-elixir/blob/main/README.md", "https://github.com/pk111and222/dioxus-mind-elixir/blob/main/README_ZH.md"), target: "_blank", rel: "noopener noreferrer", {label(lang, "Read the setup guide ↗", "查看接入指南 ↗")} } }
                }
                footer {
                    p { {label(lang, "Entirely AI-generated, with human regression testing. If you object to AI-generated software, please do not use it. AI-generated merge requests are welcome; no AI disclosure label is required.", "本项目全部由 AI 生成，已进行人工回归测试。如果你对此有意见，请勿使用。MR 接受 AI 代码，无需标注 AI 生成。")} }
                    p { {label(lang, "Web live capture is supported. Desktop live image/HTML capture is currently unavailable. This is a Rust rewrite, not full upstream API compatibility.", "支持 Web 实时捕获；Desktop 实时图片 / HTML 捕获暂不支持。本项目以 Rust 重写，不保证上游 API 全功能兼容。")} }
                    div { span { "MIT · dioxus-mind-elixir" } a { href: "https://mind-elixir.com/", "Mind Elixir ↗" } a { href: "https://github.com/ssshooter/mind-elixir-core", "Mind Elixir Core GitHub ↗" } }
                }
            }
        }
    }
}

fn count_nodes(node: &Node) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}

async fn prepare_export(
    handle: MindMapHandle,
    element: Option<Rc<MountedData>>,
    format: &str,
) -> Result<ExportFile, MindMapError> {
    let data = handle.document()?;
    match format {
        "JSON" => Ok(ExportFile {
            name: "mind-map.json".into(),
            mime: "application/json".into(),
            bytes: data.to_json()?.into_bytes(),
        }),
        "XMind" => export::xmind(&data),
        "SVG" | "PNG" | "HTML" => {
            let element = element.ok_or(MindMapError::StaleOperation)?;
            let options = ImageOptions::default();
            let svg = export::capture_svg(&element, &options).await?;
            match format {
                "SVG" => Ok(svg),
                "HTML" => export::static_html(&data.node_data.topic, &svg),
                #[cfg(all(feature = "web", target_arch = "wasm32"))]
                "PNG" => export::rasterize(&svg, export::ImageFormat::Png, &options).await,
                _ => Err(MindMapError::InvalidOperation(
                    "PNG capture requires Web".into(),
                )),
            }
        }
        _ => Err(MindMapError::InvalidOperation(
            "Unknown export format".into(),
        )),
    }
}

fn download(file: ExportFile) -> Result<(), MindMapError> {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    return file.download();
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        let _ = file;
        Err(MindMapError::InvalidOperation(
            "Use the Web playground to download files".into(),
        ))
    }
}
