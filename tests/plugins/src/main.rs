//! Independent plugin showcase, built for either Web or Desktop.
use dioxus::prelude::*;
use dioxus_mind_elixir::{
    EditableMindMap, LayoutDirection, MindMapError, Outline, Theme, Toolbar,
    plugins::{
        Labels, Plugin,
        export::{self, ExportFile, ImageOptions},
    },
    use_mind_map, use_plugins,
};
use mind_elixir_plugin_tests::fixture;
use std::rc::Rc;

fn main() {
    dioxus::launch(App);
}
#[component]
fn App() -> Element {
    let handle = match use_mind_map(fixture()) {
        Ok(handle) => handle,
        Err(error) => return rsx! {p{role:"alert","{error}"}},
    };
    let installed = use_plugins(handle, Plugin::builtins());
    if let Err(error) = installed {
        return rsx! {p{role:"alert","{error}"}};
    }
    let mut error = use_signal(String::new);
    let mut locale = use_signal(|| false);
    let mut dark = use_signal(|| false);
    let mut custom = use_signal(|| false);
    let mut font = use_signal(|| false);
    let mut readonly = use_signal(|| false);
    let mut keyboard = use_signal(|| true);
    let mut visible = use_signal(|| true);
    let mut mounted = use_signal(|| None::<Rc<MountedData>>);
    let mut files = use_signal(Vec::<ExportFile>::new);
    let mut status = use_signal(String::new);
    let on_error = use_callback(move |e: MindMapError| error.set(e.to_string()));
    let theme = if dark() {
        match dark_theme() {
            Ok(theme) => Some(theme),
            Err(e) => return rsx! {p{role:"alert","{e}"}},
        }
    } else {
        None
    };
    let customization = if custom() {
        "--dme-font-size:18px;--dme-gap-x:60px;--dme-node-border:2px solid #8569c1;--dme-toolbar-background:#ede7f9;--dme-outline-background:#f1ecfa;--dme-outline-indent:28px;--dme-command-radius:14px;--dme-marquee-background:#8569c130;--dme-link-color:#8569c1;"
    } else {
        ""
    };
    let font_css = if font() {
        use base64::Engine;
        format!(
            "@font-face{{font-family:DmeAbel;src:url(data:font/ttf;base64,{}) format('truetype')}}main{{--dme-font-family:DmeAbel,system-ui,sans-serif;}}",
            base64::engine::general_purpose::STANDARD
                .encode(include_bytes!("../fixtures/Abel-Regular.ttf"))
        )
    } else {
        String::new()
    };
    let data = handle.document();
    let json = match &data {
        Ok(data) => data.to_json(),
        Err(error) => Err(error.clone()),
    };
    let selected = match handle.selection() {
        Ok(ids) => ids.join(","),
        Err(e) => e.to_string(),
    };
    rsx! {
        style { {include_str!("../plugins.css")} }
        style {"{font_css}"}
        main {style:"{customization}",
            header {p {class:"eyebrow","DIOXUS MIND ELIXIR / STAGE 03"} h1 {"插件工作台"} p {"快捷键、拖放与共享大纲 · Rust 原生导出"}}
            div {class:"controls",
                button {id:"reset",onclick:move |_|{error.set(String::new());on_error_result(handle.set_read_only(false),error);readonly.set(false);on_error_result(handle.refresh(fixture()),error);},"Reset"}
                button {id:"language",onclick:move |_|locale.set(!locale()),"中文 / English"}
                button {id:"theme",onclick:move |_|dark.set(!dark()),"Theme"}
                button {id:"font",onclick:move |_| font.set(!font()),"Embedded font"}
                button {id:"custom",onclick:move |_|custom.set(!custom()),"CSS variables"}
                button {id:"readonly",onclick:move |_|{let next=!readonly();on_error_result(handle.set_read_only(next),error);readonly.set(next);},"Read only: {readonly}"}
                button {id:"keyboard",onclick:move |_|{let next=!keyboard();on_error_result(handle.set_plugin_enabled("builtin.Keyboard",next),error);keyboard.set(next);},"Keyboard: {keyboard}"}
                button {id:"mount",onclick:move |_|{visible.set(!visible());mounted.set(None);},"Mount / unmount"}
                for (name,layout) in [("LEFT",LayoutDirection::Left),("RIGHT",LayoutDirection::Right),("SIDE",LayoutDirection::Side),("DOWN",LayoutDirection::Down)] {
                    button {id:"layout-{name}",onclick:move |_|on_error_result(handle.apply(dioxus_mind_elixir::Operation::SetLayout(layout)).map(|_|()),error),"{name}"}
                }
            }
            Toolbar {handle,labels:if locale() {zh_labels()} else {Labels::default()},on_error}
            div {class:"workspace",
                div {id:"map",class:if dark(){"dark"}else{"light"},
                    if visible(){div {onmounted:move |event|mounted.set(Some(event.data())),EditableMindMap {handle,labels:if locale() {zh_labels()} else {Labels::default()},theme,on_error}}}
                }
                Outline {handle,labels:if locale() {zh_labels()} else {Labels::default()},on_error}
            }
            p {class:"hint","Enter 同级 · Shift+Enter 前插 · Ctrl/⌘+Enter 父级 · Tab 子级 · F2 编辑 · Ctrl/⌘+点击多选 · Shift+拖动框选"}
            div {class:"exports",
                button {id:"export-xmind",onclick:move |_|{match handle.document().and_then(|data|export::xmind(&data)){Ok(file)=>{files.set(vec![file]);status.set("XMind ready".into());},Err(e)=>on_error.call(e)}},"XMind"}
                button {id:"export-images",onclick:move |_|{
                    let element=mounted.peek().clone();status.set("Capturing…".into());error.set(String::new());
                    spawn(async move {
                        let result=async {
                            let element=element.ok_or_else(||MindMapError::InvalidOperation("Map is unmounted".into()))?;
                            let options=ImageOptions::default();let svg=export::capture_svg(&element,&options).await?;
                            let html=export::static_html(&handle.document()?.node_data.topic,&svg)?;
                            let output=vec![svg,html];
                            #[cfg(all(feature="web",target_arch="wasm32"))]
                            let mut output=output;
                            #[cfg(all(feature="web",target_arch="wasm32"))]
                            for format in [export::ImageFormat::Png,export::ImageFormat::Jpeg,export::ImageFormat::WebP] {output.push(export::rasterize(output.first().ok_or(MindMapError::StaleOperation)?,format,&options).await?);}
                            Ok::<_,MindMapError>(output)
                        }.await;
                        match result{Ok(output)=>{files.set(output);status.set("Images and static HTML ready".into());},Err(e)=>{status.set("Capture failed".into());on_error.call(e);}}
                    });
                },"SVG / PNG / JPEG / WebP / HTML"}
            }
            p {id:"export-status",role:"status","{status}"}
            for file in files() {
                ExportResult {key:"{file.name}",file}
            }
            p {id:"error",role:"alert","{error}"}
            details {summary {"共享文档 / 自动化检查"} p {id:"selection","{selected}"} pre {id:"document",match json{Ok(json)=>json,Err(e)=>e.to_string()}}}
        }
    }
}
fn on_error_result(result: Result<(), MindMapError>, mut error: Signal<String>) {
    if let Err(e) = result {
        error.set(e.to_string());
    }
}
fn dark_theme() -> Result<Theme, MindMapError> {
    let mut theme = Theme::new("Dark");
    // Constant names and values, still passed through the public checked API.
    for (name, value) in [
        ("--dme-background", "#202b31"),
        ("--dme-node-background", "#31444b"),
        ("--dme-text-color", "#e0edf0"),
        ("--dme-link-color", "#79b7a1"),
        ("--dme-node-border", "1px solid #58746e"),
        ("--dme-root-background", "#27695a"),
    ] {
        theme.set_variable(name, value)?;
    }
    Ok(theme)
}
#[component]
fn ExportResult(file: ExportFile) -> Element {
    let name = file.name.clone();
    let size = file.bytes.len();
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    let url = {
        use base64::Engine;
        format!(
            "data:{};base64,{}",
            file.mime,
            base64::engine::general_purpose::STANDARD.encode(&file.bytes)
        )
    };
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    let preview = rsx! {
        a {href:url.clone(),download:name.clone(),"Download"}
        if file.mime.starts_with("image/"){img {src:url.clone(),alt:name.clone()}}
    };
    #[cfg(not(target_arch = "wasm32"))]
    let mut saved = use_signal(String::new);
    #[cfg(not(target_arch = "wasm32"))]
    let preview = rsx! {button {onclick:move |_| {
        let result=file.save_with(|file| {
            let directory=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/plugin-artifacts/desktop");
            std::fs::create_dir_all(&directory).map_err(|e|MindMapError::InvalidOperation(e.to_string()))?;
            std::fs::write(directory.join(&file.name),&file.bytes).map_err(|e|MindMapError::InvalidOperation(e.to_string()))
        });
        saved.set(match result {Ok(())=>"Saved to target/plugin-artifacts/desktop".into(),Err(e)=>e.to_string()});
    },"Save test file"} span {role:"status","{saved}"}};
    #[cfg(all(target_arch = "wasm32", not(feature = "web")))]
    let preview = rsx! {};
    rsx! {div {class:"export-result", "data-file":name.clone(), "data-size":size,
        span {"{name} · {size} bytes"}
        {preview}
    }}
}

/// Chinese labels for the components; hosts supply their own wording like this.
fn zh_labels() -> Labels {
    let s = String::from;
    Labels {
        new_topic: s("新节点"),
        add_child: s("添加子节点"),
        add_sibling: s("添加同级"),
        insert_before: s("前插同级"),
        insert_parent: s("插入父节点"),
        delete: s("删除"),
        duplicate: s("复制"),
        fold: s("展开 / 折叠"),
        indent: s("缩进"),
        outdent: s("提升层级"),
        undo: s("撤销"),
        redo: s("重做"),
        map: s("脑图"),
        outline: s("大纲"),
        breadcrumbs: s("路径"),
        toolbar: s("脑图工具栏"),
        edit_topic: s("编辑节点"),
        collapse: s("折叠"),
        expand: s("展开"),
        toggle_children: s("显示或隐藏子节点"),
        focus_branch: s("聚焦此分支"),
        has_note: s("有备注"),
        has_image: s("有配图"),
        image_unavailable: s("图片无法显示"),
        moving: s("拖动节点 · Esc 取消"),
        drop_here: s("释放以移动"),
    }
}
