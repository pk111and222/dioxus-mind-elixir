//! Rust file encoders. No upstream scripts or runtime are embedded.
use crate::{MindMapData, MindMapError, Node};
use serde_json::{Value, json};
use std::io::{Cursor, Write};

fn failure(error: impl std::fmt::Display) -> MindMapError {
    MindMapError::InvalidOperation(format!("Export failed: {error}"))
}
/// A complete file, passed to an application-owned save/download sink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportFile {
    /// Suggested filename; hosts may replace it.
    pub name: String,
    /// MIME content type.
    pub mime: String,
    /// File payload.
    pub bytes: Vec<u8>,
}
impl ExportFile {
    /// Save using a replaceable host transport (native filesystem, dialog, download).
    pub fn save_with(
        &self,
        sink: impl FnOnce(&Self) -> Result<(), MindMapError>,
    ) -> Result<(), MindMapError> {
        sink(self)
    }
    /// Download with an object URL, released immediately after the click.
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    pub fn download(&self) -> Result<(), MindMapError> {
        web::download(self)
    }
}
/// Encode a Zen/modern XMind ZIP with content.json, metadata.json and manifest.json.
/// Exports the entire plain-text tree, including collapsed descendants. Arbitrary
/// Mind Elixir metadata and CSS are not XMind fields and are not converted.
pub fn xmind(data: &MindMapData) -> Result<ExportFile, MindMapError> {
    data.validate()?;
    fn topic(node: &Node, root: bool, direction: crate::LayoutDirection) -> Value {
        let mut result = json!({"id":node.id,"class":"topic","title":node.topic});
        if !node.children.is_empty() {
            result["children"] = json!({"attached":node.children.iter().map(|n|topic(n,false,direction)).collect::<Vec<_>>()});
        }
        if !node.expanded {
            result["branch"] = json!("folded");
        }
        if let Some(note) = &node.note {
            result["notes"] = json!({"plain":{"content":note}});
        }
        if root {
            result["structureClass"] = json!(match direction {
                crate::LayoutDirection::Down => "org.xmind.ui.logic.down",
                crate::LayoutDirection::Left => "org.xmind.ui.logic.left",
                crate::LayoutDirection::Right => "org.xmind.ui.logic.right",
                crate::LayoutDirection::Side => "org.xmind.ui.map.unbalanced",
            });
        }
        if let Some(style) = &node.style {
            let mut properties = serde_json::Map::new();
            for (name, value) in [
                ("fo:font-size", &style.font_size),
                ("fo:font-family", &style.font_family),
                ("fo:font-weight", &style.font_weight),
                ("fo:color", &style.color),
                ("svg:fill", &style.background),
            ] {
                if let Some(value) = value {
                    properties.insert(name.into(), json!(value));
                }
            }
            result["style"] = json!({"properties":properties});
        }
        result
    }
    let content = json!([{"id":"dme-sheet","class":"sheet","title":data.node_data.topic,"rootTopic":topic(&data.node_data,true,data.direction)}]);
    let metadata = json!({"creator":{"name":"dioxus-mind-elixir","version":env!("CARGO_PKG_VERSION")},"activeSheetId":"dme-sheet"});
    let manifest = json!({"file-entries":{"content.json":{},"metadata.json":{}}});
    let mut archive = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, value) in [
        ("content.json", content),
        ("metadata.json", metadata),
        ("manifest.json", manifest),
    ] {
        archive
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .map_err(failure)?;
        archive
            .write_all(&serde_json::to_vec(&value).map_err(failure)?)
            .map_err(failure)?;
    }
    Ok(ExportFile {
        name: "mind-map.xmind".into(),
        mime: "application/x-xmind".into(),
        bytes: archive.finish().map_err(failure)?.into_inner(),
    })
}
/// Wrap a captured standalone SVG in a static, offline HTML document.
/// No editor or scripts are included. SVG must come from a trusted capture source.
pub fn static_html(title: &str, svg: &ExportFile) -> Result<ExportFile, MindMapError> {
    if svg.mime != "image/svg+xml" {
        return Err(failure("Expected an SVG capture"));
    }
    let body = std::str::from_utf8(&svg.bytes).map_err(failure)?;
    Ok(ExportFile{name:"mind-map.html".into(),mime:"text/html".into(),bytes:format!("<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; style-src 'unsafe-inline'; img-src data:; font-src data:;\"><title>{}</title><style>body{{margin:0;overflow:auto}}svg{{display:block;max-width:none}}</style></head><body>{body}</body></html>",escape(title)).into_bytes()})
}
pub(crate) fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
/// Raster output encoding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageFormat {
    /// Lossless PNG.
    Png,
    /// JPEG with caller-defined quality.
    Jpeg,
    /// WebP if supported by the browser's encoder.
    WebP,
}
/// Capture and raster settings. Dimensions are measured from the live view.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageOptions {
    /// Device-independent raster scale; finite, in 0.1..=4.
    pub scale: f64,
    /// JPEG/WebP quality, in 0..=1.
    pub quality: f64,
    /// Opaque canvas fallback when the live map background is transparent.
    pub background: String,
}
impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            scale: 1.0,
            quality: 0.9,
            background: "#ffffff".into(),
        }
    }
}
impl ImageOptions {
    /// Validate bounds before allocating a canvas.
    pub fn validate(&self) -> Result<(), MindMapError> {
        if !self.scale.is_finite()
            || !(0.1..=4.0).contains(&self.scale)
            || !self.quality.is_finite()
            || !(0.0..=1.0).contains(&self.quality)
            || self.background.trim().is_empty()
        {
            return Err(failure("Invalid image scale, quality or background"));
        }
        Ok(())
    }
}
/// Capture a mounted Web map, resolving inherited CSS and loaded fonts.
/// Other renderers receive a typed error and may provide their own capture sink.
/// Call while the map is mounted and not being edited or animated.
#[cfg(not(all(feature = "web", target_arch = "wasm32")))]
pub async fn capture_svg(
    _element: &dioxus::prelude::MountedData,
    options: &ImageOptions,
) -> Result<ExportFile, MindMapError> {
    options.validate()?;
    Err(failure(
        "Live CSS capture requires the web feature and a Web renderer; use a host capture adapter on Desktop",
    ))
}
#[cfg(all(feature = "web", target_arch = "wasm32"))]
pub use web::{capture_svg, rasterize};
#[cfg(all(feature = "web", target_arch = "wasm32"))]
mod web;
