use super::{ExportFile, ImageFormat, ImageOptions, escape, failure};
use crate::MindMapError;
use base64::{Engine, engine::general_purpose::STANDARD};
use dioxus::prelude::MountedData;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Element, HtmlCanvasElement, HtmlImageElement};

fn js(error: impl Into<JsValue>) -> MindMapError {
    failure(format!("{:?}", error.into()))
}
fn window() -> Result<web_sys::Window, MindMapError> {
    web_sys::window().ok_or_else(|| failure("No browser window"))
}
fn document() -> Result<web_sys::Document, MindMapError> {
    window()?
        .document()
        .ok_or_else(|| failure("No browser document"))
}
fn computed(element: &Element) -> Result<web_sys::CssStyleDeclaration, MindMapError> {
    window()?
        .get_computed_style(element)
        .map_err(js)?
        .ok_or_else(|| failure("No computed styles"))
}
// Layout has already been resolved by the browser. Inlining every computed
// property preserves inherited variables without shipping the host stylesheet.
fn inline(live: &Element, clone: &Element) -> Result<(), MindMapError> {
    let style = computed(live)?;
    let mut css = String::new();
    for i in 0..style.length() {
        let name = style.item(i);
        let value = style.get_property_value(&name).map_err(js)?;
        // URLs are not silently left pointing at the original application's origin.
        if value.contains("url(") && !value.contains("url(\"data:") && !value.contains("url(data:")
        {
            return Err(failure(format!(
                "External CSS resource in {name}; embed it as a data URL before export"
            )));
        }
        css.push_str(&format!("{name}:{value};"));
    }
    clone.set_attribute("style", &css).map_err(js)?;
    let a = live.children();
    let b = clone.children();
    if a.length() != b.length() {
        return Err(failure("View changed during capture"));
    }
    for i in 0..a.length() {
        if let (Some(a), Some(b)) = (a.item(i), b.item(i)) {
            inline(&a, &b)?;
        }
    }
    Ok(())
}
async fn font_css() -> Result<String, MindMapError> {
    let sheets = document()?.style_sheets();
    let mut rules = Vec::new();
    for i in 0..sheets.length() {
        let Some(sheet) = sheets.item(i) else {
            continue;
        };
        let sheet = sheet.dyn_into::<web_sys::CssStyleSheet>().map_err(js)?;
        let list=sheet.css_rules().map_err(|_|failure("Cannot read cross-origin font stylesheet; serve fonts and styles with CORS on the same origin"))?;
        for i in 0..list.length() {
            if let Some(rule) = list.item(i) {
                let css = rule.css_text();
                if css.starts_with("@font-face") {
                    rules.push(css);
                }
            }
        }
    }
    let mut output = String::new();
    for mut css in rules {
        while let Some(start) = css.find("url(") {
            let rest = css
                .get(start + 4..)
                .ok_or_else(|| failure("Invalid font URL"))?;
            let end = rest.find(')').ok_or_else(|| failure("Invalid font URL"))?;
            let url = rest
                .get(..end)
                .ok_or_else(|| failure("Invalid font URL"))?
                .trim()
                .trim_matches(['\'', '"']);
            if url.starts_with("data:") {
                // Preserve inline fonts; advance by processing the remaining rule.
                output.push_str(
                    css.get(..start + 4 + end + 1)
                        .ok_or_else(|| failure("Invalid font rule"))?,
                );
                css = css
                    .get(start + 4 + end + 1..)
                    .ok_or_else(|| failure("Invalid font rule"))?
                    .into();
                continue;
            }
            let response = JsFuture::from(window()?.fetch_with_str(url))
                .await
                .map_err(js)?
                .dyn_into::<web_sys::Response>()
                .map_err(js)?;
            if !response.ok() {
                return Err(failure(format!(
                    "Font request failed: {}",
                    response.status()
                )));
            }
            let bytes = JsFuture::from(response.array_buffer().map_err(js)?)
                .await
                .map_err(js)?;
            let encoded = STANDARD.encode(js_sys::Uint8Array::new(&bytes).to_vec());
            let replacement = format!("url(\"data:font/woff2;base64,{encoded}\")");
            let before = css
                .get(..start)
                .ok_or_else(|| failure("Invalid font rule"))?;
            let after = css
                .get(start + 4 + end + 1..)
                .ok_or_else(|| failure("Invalid font rule"))?;
            // Consume the replacement now so its data URL is not processed again.
            output.push_str(before);
            output.push_str(&replacement);
            css = after.into();
        }
        output.push_str(&css);
    }
    Ok(output)
}
/// Capture actual HTML layout into a self-contained SVG foreignObject image.
/// Full canvas is exported; viewport pan and zoom are removed, editing and
/// selection affordances are omitted. Loaded custom fonts are embedded.
pub async fn capture_svg(
    mounted: &MountedData,
    options: &ImageOptions,
) -> Result<ExportFile, MindMapError> {
    options.validate()?;
    let element = mounted
        .downcast::<Element>()
        .ok_or_else(|| failure("Mounted element is not a Web DOM element"))?;
    if !element.is_connected() {
        return Err(failure("Map has been unmounted"));
    }
    JsFuture::from(document()?.fonts().ready().map_err(js)?)
        .await
        .map_err(js)?;
    let map = if element.class_list().contains("dme-map") {
        element.clone()
    } else {
        element
            .query_selector(".dme-map")
            .map_err(js)?
            .ok_or_else(|| failure("Expected .dme-map"))?
    };
    let canvas = map
        .query_selector(".dme-canvas")
        .map_err(js)?
        .ok_or_else(|| failure("Expected a mounted .dme-map"))?;
    if element.query_selector("textarea").map_err(js)?.is_some() {
        return Err(failure("Finish editing before export"));
    }
    let before = canvas.inner_html();
    let width = f64::from(
        canvas
            .dyn_ref::<web_sys::HtmlElement>()
            .ok_or_else(|| failure("No canvas dimensions"))?
            .offset_width(),
    );
    let height = f64::from(
        canvas
            .dyn_ref::<web_sys::HtmlElement>()
            .ok_or_else(|| failure("No canvas dimensions"))?
            .offset_height(),
    );
    if width <= 0.0 || height <= 0.0 || width * height * options.scale.powi(2) > 64_000_000.0 {
        return Err(failure(
            "Invalid or oversized image (64 million pixel limit)",
        ));
    }
    let clone = canvas
        .clone_node_with_deep(true)
        .map_err(js)?
        .dyn_into::<Element>()
        .map_err(js)?;
    inline(&canvas, &clone)?;
    let clone_style = clone
        .get_attribute("style")
        .ok_or_else(|| failure("Missing inline style"))?;
    clone.set_attribute("style",&format!("{clone_style};transform:none;min-width:0;min-height:0;width:{width}px;height:{height}px;position:relative;")).map_err(js)?;
    let remove = clone
        .query_selector_all(".dme-collapse,.dme-focus-marker,.dme-scale-probe")
        .map_err(js)?;
    for i in 0..remove.length() {
        if let Some(node) = remove.item(i) {
            if let Some(parent) = node.parent_node() {
                parent.remove_child(&node).map_err(js)?;
            }
        }
    }
    let topics = clone.query_selector_all(".dme-topic").map_err(js)?;
    for i in 0..topics.length() {
        if let Some(node) = topics.item(i) {
            if let Some(element) = node.dyn_ref::<Element>() {
                let css = element
                    .get_attribute("style")
                    .ok_or_else(|| failure("Missing node style"))?;
                element
                    .set_attribute("style", &format!("{css};outline:none;"))
                    .map_err(js)?;
            }
        }
    }
    let fonts = font_css().await?;
    if !element.is_connected() || canvas.inner_html() != before {
        return Err(failure("Map changed or unmounted while capturing; retry"));
    }
    let background = computed(&map)?
        .get_property_value("background-color")
        .map_err(js)?;
    let background = if background == "rgba(0, 0, 0, 0)" || background == "transparent" {
        options.background.clone()
    } else {
        background
    };
    let xml = web_sys::XmlSerializer::new()
        .map_err(js)?
        .serialize_to_string(&clone)
        .map_err(js)?;
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><foreignObject width=\"100%\" height=\"100%\"><div xmlns=\"http://www.w3.org/1999/xhtml\" style=\"width:{width}px;height:{height}px;background:{}\"><style>{}</style>{xml}</div></foreignObject></svg>",
        escape(&background),
        escape(&fonts)
    );
    Ok(ExportFile {
        name: "mind-map.svg".into(),
        mime: "image/svg+xml".into(),
        bytes: svg.into_bytes(),
    })
}
/// Rasterize the captured SVG with the browser's native image/canvas codec.
/// Detects unsupported codecs instead of returning PNG bytes with a WebP suffix.
pub async fn rasterize(
    svg: &ExportFile,
    format: ImageFormat,
    options: &ImageOptions,
) -> Result<ExportFile, MindMapError> {
    options.validate()?;
    if svg.mime != "image/svg+xml" {
        return Err(failure("Expected an SVG capture"));
    }
    let image = HtmlImageElement::new().map_err(js)?;
    image.set_src(&format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(&svg.bytes)
    ));
    JsFuture::from(image.decode()).await.map_err(js)?;
    let width = (f64::from(image.natural_width()) * options.scale).round();
    let height = (f64::from(image.natural_height()) * options.scale).round();
    if width <= 0.0 || height <= 0.0 || width * height > 64_000_000.0 {
        return Err(failure("Invalid raster dimensions"));
    }
    let canvas = document()?
        .create_element("canvas")
        .map_err(js)?
        .dyn_into::<HtmlCanvasElement>()
        .map_err(js)?;
    canvas.set_width(width as u32);
    canvas.set_height(height as u32);
    let context = canvas
        .get_context("2d")
        .map_err(js)?
        .ok_or_else(|| failure("No 2D canvas"))?
        .dyn_into::<web_sys::CanvasRenderingContext2d>()
        .map_err(js)?;
    context.set_fill_style_str(&options.background);
    context.fill_rect(0.0, 0.0, width, height);
    context
        .draw_image_with_html_image_element_and_dw_and_dh(&image, 0.0, 0.0, width, height)
        .map_err(js)?;
    let (extension, mime) = match format {
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::Jpeg => ("jpg", "image/jpeg"),
        ImageFormat::WebP => ("webp", "image/webp"),
    };
    let url = canvas
        .to_data_url_with_type_and_encoder_options(mime, &JsValue::from_f64(options.quality))
        .map_err(js)?;
    let expected = format!("data:{mime};base64,");
    let encoded = url
        .strip_prefix(&expected)
        .ok_or_else(|| failure(format!("Browser does not support {mime}")))?;
    Ok(ExportFile {
        name: format!("mind-map.{extension}"),
        mime: mime.into(),
        bytes: STANDARD.decode(encoded).map_err(failure)?,
    })
}
pub(super) fn download(file: &ExportFile) -> Result<(), MindMapError> {
    let parts = js_sys::Array::new();
    parts.push(&js_sys::Uint8Array::from(file.bytes.as_slice()));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type(&file.mime);
    let blob =
        web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(js)?;
    let url = web_sys::Url::create_object_url_with_blob(&blob).map_err(js)?;
    let result = (|| {
        let anchor = document()?
            .create_element("a")
            .map_err(js)?
            .dyn_into::<web_sys::HtmlAnchorElement>()
            .map_err(js)?;
        anchor.set_href(&url);
        anchor.set_download(&file.name);
        anchor.click();
        Ok(())
    })();
    web_sys::Url::revoke_object_url(&url).map_err(js)?;
    result
}
