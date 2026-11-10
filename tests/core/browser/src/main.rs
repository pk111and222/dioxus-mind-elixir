//! WebDriver regression runner. No execute-script endpoint or handwritten JS.
use base64::Engine;
use reqwest::blocking::Client;
use serde_json::{Value, json};
use std::{
    error::Error,
    fs,
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

type Result<T = ()> = std::result::Result<T, Box<dyn Error>>;
const ELEMENT: &str = "element-6066-11e4-a52e-4f735466cecf";

struct Driver {
    client: Client,
    endpoint: String,
    session: String,
    artifacts: PathBuf,
}
impl Driver {
    fn create() -> Result<Self> {
        let endpoint =
            std::env::var("DME_WEBDRIVER_URL").unwrap_or_else(|_| "http://localhost:9515".into());
        let browser = std::env::var("DME_BROWSER").unwrap_or_else(|_| "chrome".into());
        let capabilities = if browser == "safari" {
            json!({"browserName":"safari"})
        } else {
            json!({"browserName":"chrome","goog:chromeOptions":{"args":["--headless=new","--window-size=1600,1100","--disable-gpu","--no-sandbox"]}})
        };
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;
        let response: Value = client
            .post(format!("{endpoint}/session"))
            .json(&json!({"capabilities":{"alwaysMatch":capabilities}}))
            .send()?
            .json()?;
        let session = response
            .pointer("/value/sessionId")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("WebDriver session failed: {response}"))?
            .to_owned();
        let artifacts = PathBuf::from(
            std::env::var("DME_E2E_ARTIFACTS").unwrap_or_else(|_| "target/core-e2e".into()),
        );
        fs::create_dir_all(&artifacts)?;
        Ok(Self {
            client,
            endpoint,
            session,
            artifacts,
        })
    }
    fn request(&self, method: &str, path: &str, body: Option<Value>) -> Result<Value> {
        let url = format!("{}/session/{}{}", self.endpoint, self.session, path);
        let request = match method {
            "GET" => self.client.get(url),
            "DELETE" => self.client.delete(url),
            _ => self.client.post(url),
        };
        let request = if let Some(body) = body {
            request.json(&body)
        } else {
            request
        };
        let response: Value = request.send()?.json()?;
        let value = response
            .get("value")
            .ok_or("WebDriver response has no value")?;
        if value.get("error").is_some() {
            return Err(format!("WebDriver {method} {path}: {value}").into());
        }
        Ok(value.clone())
    }
    fn elements(&self, selector: &str) -> Result<Vec<String>> {
        let value = self.request(
            "POST",
            "/elements",
            Some(json!({"using":"css selector","value":selector})),
        )?;
        value
            .as_array()
            .ok_or("Expected elements array")?
            .iter()
            .map(|item| {
                item.get(ELEMENT)
                    .and_then(Value::as_str)
                    .map(str::to_owned)
                    .ok_or_else(|| "Missing element ID".into())
            })
            .collect()
    }
    fn element(&self, selector: &str) -> Result<String> {
        self.elements(selector)?
            .first()
            .cloned()
            .ok_or_else(|| format!("Missing element: {selector}").into())
    }
    fn click(&self, selector: &str) -> Result {
        let id = self.element(selector)?;
        self.request("POST", &format!("/element/{id}/click"), Some(json!({})))?;
        Ok(())
    }
    fn text(&self, selector: &str) -> Result<String> {
        let id = self.element(selector)?;
        Ok(self
            .request("GET", &format!("/element/{id}/text"), None)?
            .as_str()
            .ok_or("Expected text")?
            .into())
    }
    fn button(&self, label: &str) -> Result {
        for id in self.elements("button")? {
            if self
                .request("GET", &format!("/element/{id}/text"), None)?
                .as_str()
                == Some(label)
            {
                self.request("POST", &format!("/element/{id}/click"), Some(json!({})))?;
                return Ok(());
            }
        }
        Err(format!("Missing button: {label}").into())
    }
    fn attribute(&self, selector: &str, name: &str) -> Result<Value> {
        let id = self.element(selector)?;
        self.request("GET", &format!("/element/{id}/attribute/{name}"), None)
    }
    fn css(&self, selector: &str, name: &str) -> Result<Value> {
        let id = self.element(selector)?;
        self.request("GET", &format!("/element/{id}/css/{name}"), None)
    }
    fn rect(&self, selector: &str) -> Result<Rect> {
        let id = self.element(selector)?;
        let value = self.request("GET", &format!("/element/{id}/rect"), None)?;
        Ok(Rect {
            x: number(&value, "x")?,
            y: number(&value, "y")?,
            width: number(&value, "width")?,
            height: number(&value, "height")?,
        })
    }
    fn keys(&self, selector: &str, text: &str) -> Result {
        let id = self.element(selector)?;
        self.request(
            "POST",
            &format!("/element/{id}/value"),
            Some(json!({"text":text})),
        )?;
        Ok(())
    }
    fn wait(&self, mut condition: impl FnMut() -> Result<bool>) -> Result {
        let started = Instant::now();
        while started.elapsed() < Duration::from_secs(15) {
            if condition()? {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(80));
        }
        Err("Timed out waiting for stable rendered state".into())
    }
    fn screenshot(&self, name: &str) -> Result {
        let value = self.request("GET", "/screenshot", None)?;
        let data = base64::engine::general_purpose::STANDARD
            .decode(value.as_str().ok_or("Missing screenshot")?)?;
        fs::write(self.artifacts.join(format!("{name}.png")), data)?;
        Ok(())
    }
    fn check_links(&self) -> Result {
        let canvas = self.rect("#primary .dme-canvas")?;
        let scale: f64 = self
            .attribute("#primary .dme-canvas", "data-scale")?
            .as_str()
            .ok_or("Missing scale")?
            .parse()?;
        let direction = self.attribute("#primary .dme-map", "data-layout")?;
        let paths = self.elements("#primary .dme-link")?;
        ensure(!paths.is_empty(), "No real SVG connectors")?;
        let nodes = self.elements("#primary .dme-topic")?;
        let mut rectangles = Vec::new();
        for id in nodes {
            let value = self.request("GET", &format!("/element/{id}/rect"), None)?;
            let rect = Rect {
                x: (number(&value, "x")? - canvas.x) / scale,
                y: (number(&value, "y")? - canvas.y) / scale,
                width: number(&value, "width")?,
                height: number(&value, "height")?,
            };
            rectangles.push(rect);
        }
        let mut endpoints = Vec::new();
        for id in paths {
            let value = self.request("GET", &format!("/element/{id}/attribute/d"), None)?;
            let path = value.as_str().ok_or("Empty SVG path")?;
            for subpath in path.split('M').skip(1) {
                let tokens: Vec<_> = subpath.split_whitespace().collect();
                let last = tokens.last().ok_or("Empty subpath")?.parse::<f64>()?;
                if direction.as_str() == Some("down") {
                    let index = tokens
                        .iter()
                        .rposition(|token| *token == "V")
                        .ok_or("Missing vertical endpoint")?;
                    let x = if tokens.len() == 5 {
                        tokens.first().ok_or("Missing x")?.parse::<f64>()?
                    } else {
                        tokens
                            .get(index.checked_sub(2).ok_or("Malformed rounded path")?)
                            .ok_or("Missing x")?
                            .parse::<f64>()?
                    };
                    endpoints.push((x, last));
                } else {
                    let x = tokens
                        .get(tokens.len().checked_sub(2).ok_or("Malformed path")?)
                        .ok_or("Missing x")?
                        .parse::<f64>()?;
                    endpoints.push((x, last));
                }
            }
        }
        ensure(
            endpoints.len() + 1 == rectangles.len(),
            "Connector count does not match visible tree",
        )?;
        for (x, y) in endpoints {
            ensure(
                rectangles.iter().any(|rect| {
                    if direction.as_str() == Some("down") {
                        close(x, rect.x + rect.width / 2.0) && close(y, rect.y)
                    } else {
                        (close(x, rect.x) || close(x, rect.x + rect.width))
                            && close(y, rect.y + rect.height / 2.0)
                    }
                }),
                &format!("Connector endpoint ({x}, {y}) misses measured node edge: {rectangles:?}"),
            )?;
        }
        Ok(())
    }
    fn check_collapse_controls(&self) -> Result {
        for host in ["#primary", "#secondary"] {
            ensure(
                self.elements(&format!("{host} [data-root='true'] .dme-collapse"))?
                    .is_empty(),
                "Root must not show a branch collapse control",
            )?;
            let scale: f64 = self
                .attribute(&format!("{host} .dme-canvas"), "data-scale")?
                .as_str()
                .ok_or("Missing scale")?
                .parse()?;
            for id in ["design", "build"] {
                let selector = format!("{host} [data-node-id='{id}']");
                let topic = self.rect(&selector)?;
                let button = self.rect(&format!("{selector} > .dme-collapse"))?;
                let flow = self.attribute(&selector, "data-flow")?;
                let center_x = button.x + button.width * scale / 2.0;
                let center_y = button.y + button.height * scale / 2.0;
                ensure(
                    button.width >= 24.0 && close(button.width, button.height),
                    "Collapse target must be square and at least 24px",
                )?;
                match flow.as_str() {
                    Some("left") => {
                        ensure(
                            close(center_y, topic.y + topic.height * scale / 2.0),
                            "Left collapse control is not vertically centered",
                        )?;
                        ensure(
                            button.x + button.width * scale <= topic.x - 4.0 * scale,
                            "Left control overlaps node border",
                        )?;
                    }
                    Some("down") => {
                        ensure(
                            close(center_x, topic.x + topic.width * scale / 2.0),
                            "Down collapse control is not horizontally centered",
                        )?;
                        ensure(
                            button.y >= topic.y + topic.height * scale + 4.0 * scale,
                            "Down control overlaps node border",
                        )?;
                    }
                    _ => {
                        ensure(
                            close(center_y, topic.y + topic.height * scale / 2.0),
                            "Right collapse control is not vertically centered",
                        )?;
                        ensure(
                            button.x >= topic.x + topic.width * scale + 4.0 * scale,
                            "Right control overlaps node border",
                        )?;
                    }
                }
                for other in self.elements(&format!("{host} .dme-topic"))? {
                    let value = self.request("GET", &format!("/element/{other}/rect"), None)?;
                    let (x, y, w, h) = (
                        number(&value, "x")?,
                        number(&value, "y")?,
                        number(&value, "width")? * scale,
                        number(&value, "height")? * scale,
                    );
                    ensure(
                        button.x + button.width * scale <= x
                            || button.x >= x + w
                            || button.y + button.height * scale <= y
                            || button.y >= y + h,
                        "Collapse control overlaps a visible node",
                    )?;
                }
            }
        }
        Ok(())
    }
    fn run(&self) -> Result {
        let url = std::env::var("DME_APP_URL").unwrap_or_else(|_| "http://localhost:8088".into());
        self.request("POST", "/url", Some(json!({"url":url})))?;
        self.wait(|| {
            Ok(self.elements("#primary .dme-topic")?.len() == 8
                && !self.elements("#primary .dme-link")?.is_empty())
        })?;
        for label in ["RIGHT", "LEFT", "SIDE", "DOWN"] {
            self.button(label)?;
            self.wait(|| Ok(self.check_links().is_ok()))?;
            self.check_links()?;
            self.check_collapse_controls()?;
            self.button("紧凑")?;
            self.wait(|| Ok(self.check_collapse_controls().is_ok() && self.check_links().is_ok()))?;
            self.screenshot(&format!("collapse-compact-{label}"))?;
            self.button("紧凑")?;
            self.wait(|| Ok(self.check_links().is_ok()))?;
            self.click("#primary [data-node-id='design'] > .dme-collapse")?;
            self.wait(|| Ok(self.elements("#primary [data-node-id='css']")?.is_empty()))?;
            ensure(
                self.attribute(
                    "#primary [data-node-id='design'] > .dme-collapse",
                    "aria-expanded",
                )?
                .as_str()
                    == Some("false"),
                "Collapsed control lacks expanded state",
            )?;
            self.check_collapse_controls()?;
            self.screenshot(&format!("collapse-collapsed-{label}"))?;
            self.keys(
                "#primary [data-node-id='design'] > .dme-collapse",
                "\u{e007}",
            )?;
            self.wait(|| {
                Ok(!self.elements("#primary [data-node-id='css']")?.is_empty()
                    && self.check_links().is_ok())
            })?;
            self.button("Zoom +")?;

            self.wait(|| {
                let result = self.check_links();

                Ok(self
                    .attribute("#primary .dme-canvas", "data-scale")?
                    .as_str()
                    == Some("1.25")
                    && result.is_ok())
            })?;
            self.check_collapse_controls()?;
            self.button("视口重置")?;
            self.wait(|| {
                Ok(self
                    .attribute("#primary .dme-canvas", "data-scale")?
                    .as_str()
                    == Some("1")
                    && self.check_links().is_ok())
            })?;
            self.screenshot(&format!("layout-{label}"))?;
        }
        self.button("RIGHT")?;
        self.wait(|| Ok(self.check_links().is_ok()))?;
        let secondary_font = self.css("#secondary .dme-topic", "font-size")?;
        let before = self.rect("#primary [data-node-id='css']")?;
        self.button("CSS 间距/字体")?;
        self.wait(|| {
            Ok(
                self.rect("#primary [data-node-id='css']")?.height > before.height
                    && self.check_links().is_ok(),
            )
        })?;
        ensure(
            self.css("#secondary .dme-topic", "font-size")? == secondary_font,
            "CSS leaked across instances",
        )?;
        self.button("主题")?;
        ensure(
            self.css("#primary .dme-map", "background-color")?
                != self.css("#secondary .dme-map", "background-color")?,
            "Theme override missing",
        )?;
        self.check_collapse_controls()?;
        self.screenshot("collapse-dark-spacious")?;
        self.click("#primary [data-node-id='css']")?;
        self.keys("#primary [data-node-id='css']", "\u{e032}")?; // F2
        self.wait(|| Ok(self.elements("#primary .dme-editor")?.len() == 1))?;
        let editor = self.element("#primary .dme-editor")?;
        self.wait(|| {
            Ok(self
                .request("GET", "/element/active", None)?
                .get(ELEMENT)
                .and_then(Value::as_str)
                == Some(editor.as_str()))
        })?;
        self.keys("#primary .dme-editor", "\u{e03d}a\u{e000}\u{e003}")?;
        self.keys("#primary .dme-editor", "中文核心编辑")?;
        self.keys("#primary .dme-editor", "\u{e008}\u{e007}\u{e000}")?; // Shift+Enter, release
        self.keys("#primary .dme-editor", "Second line\u{e004}")?; // Tab commit
        self.wait(|| {
            Ok(self.elements("#primary .dme-editor")?.is_empty()
                && self
                    .text("#secondary [data-node-id='css']")?
                    .contains("中文核心编辑"))
        })?;
        let topic = self.element("#primary [data-node-id='css']")?;
        self.wait(|| {
            Ok(self
                .request("GET", "/element/active", None)?
                .get(ELEMENT)
                .and_then(Value::as_str)
                == Some(topic.as_str()))
        })?;
        self.button("Undo")?;
        self.wait(|| {
            Ok(self
                .text("#primary [data-node-id='css']")?
                .contains("外部定制"))
        })?;
        self.button("Redo")?;
        self.wait(|| {
            Ok(self
                .text("#primary [data-node-id='css']")?
                .contains("Second line"))
        })?;
        self.keys("#primary [data-node-id='css']", "\u{e032}")?;
        self.wait(|| Ok(!self.elements("#primary .dme-editor")?.is_empty()))?;
        self.keys("#primary .dme-editor", "\u{e03d}a\u{e000}\u{e003}")?;
        self.keys("#primary .dme-editor", "cancel me\u{e00c}")?;
        self.wait(|| Ok(self.elements("#primary .dme-editor")?.is_empty()))?;
        ensure(
            !self
                .text("#primary [data-node-id='css']")?
                .contains("cancel me"),
            "Escape committed draft",
        )?;
        self.click("#primary [data-node-id='design'] .dme-collapse")?;
        self.wait(|| Ok(self.elements("#primary [data-node-id='css']")?.is_empty()))?;
        self.click("#primary [data-node-id='design']")?;
        self.button("添加子节点")?;
        self.wait(|| Ok(self.elements("#primary [data-node-id='new-1']")?.len() == 1))?;
        self.button("Undo")?;
        self.wait(|| Ok(self.elements("#primary [data-node-id='css']")?.is_empty()))?;
        self.button("Redo")?;
        self.wait(|| Ok(self.check_links().is_ok()))?;
        self.button("只读")?;
        self.button("添加子节点")?;
        ensure(
            self.text("#status")?.contains("read-only"),
            "Read-only bypassed",
        )?;
        self.button("允许编辑")?;
        self.button("卸载第二实例")?;
        self.wait(|| Ok(self.elements("#secondary")?.is_empty()))?;
        self.button("挂载第二实例")?;
        self.wait(|| Ok(!self.elements("#secondary .dme-link")?.is_empty()))?;
        self.click("#primary [data-node-id='css']")?;
        self.keys("#primary [data-node-id='css']", "\u{e032}")?;
        self.wait(|| Ok(!self.elements("#primary .dme-editor")?.is_empty()))?;
        // Rapid document/layout/unmount changes must cancel old measurements
        // and drafts, including replacement documents with identical IDs.
        self.button("Refresh")?;
        self.wait(|| Ok(self.elements("#primary .dme-editor")?.is_empty()))?;
        for label in ["LEFT", "DOWN", "RIGHT", "SIDE", "RIGHT"] {
            self.button(label)?;
        }
        self.wait(|| {
            Ok(self.elements("#primary .dme-topic")?.len() == 8 && self.check_links().is_ok())
        })?;
        let before_pan = self.rect("#primary [data-node-id='root']")?;
        let view = self.rect("#primary .dme-map")?;
        self.request("POST", "/actions", Some(json!({"actions":[{"type":"pointer","id":"mouse","parameters":{"pointerType":"mouse"},"actions":[
            {"type":"pointerMove","duration":0,"origin":"viewport","x":(view.x + view.width - 50.0) as i64,"y":(view.y + 40.0) as i64},
            {"type":"pointerDown","button":0},
            {"type":"pointerMove","duration":150,"origin":"pointer","x":30,"y":20},
            {"type":"pointerUp","button":0}
        ]}]})))?;
        self.wait(|| {
            Ok(
                (self.rect("#primary [data-node-id='root']")?.x - before_pan.x - 30.0).abs() < 2.0
                    && self.check_links().is_ok(),
            )
        })?;
        self.request("DELETE", "/actions", None)?;
        self.button("居中")?;
        self.wait(|| {
            let root = self.rect("#primary [data-node-id='root']")?;
            let view = self.rect("#primary .dme-map")?;
            Ok(close(root.x + root.width / 2.0, view.x + view.width / 2.0)
                && close(root.y + root.height / 2.0, view.y + view.height / 2.0)
                && self.check_links().is_ok())
        })?;
        self.button("视口重置")?;
        self.wait(|| Ok(self.check_links().is_ok()))?;
        self.screenshot("core-success")?;
        fs::write(
            self.artifacts.join("result.txt"),
            "PASS: four layouts, measured connector endpoints, CSS isolation, shared editing, focus, multiline Unicode, Escape, history, auto-expansion, readonly, unmount/remount, refresh, zoom, pan, center, focus restoration, stale draft cancellation, directional collapse placement/compact/zoom/theme/custom size/click targets\nIME candidate confirmation requires real OS input.\n",
        )?;
        Ok(())
    }
}
#[derive(Debug)]
struct Rect {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}
fn number(value: &Value, key: &str) -> Result<f64> {
    value
        .get(key)
        .and_then(Value::as_f64)
        .ok_or_else(|| format!("Missing numeric {key}").into())
}
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 2.0
}
fn ensure(condition: bool, message: &str) -> Result {
    if condition {
        Ok(())
    } else {
        Err(message.to_owned().into())
    }
}
fn main() -> Result {
    let driver = match Driver::create() {
        Ok(driver) => driver,
        Err(error) => {
            let path = PathBuf::from(
                std::env::var("DME_E2E_ARTIFACTS").unwrap_or_else(|_| "target/core-e2e".into()),
            );
            fs::create_dir_all(&path)?;
            fs::write(path.join("startup-failure.txt"), error.to_string())?;
            return Err(error);
        }
    };
    for name in [
        "result.txt",
        "failure.txt",
        "failure.png",
        "startup-failure.txt",
    ] {
        let path = driver.artifacts.join(name);
        if path.exists() {
            fs::remove_file(path)?;
        }
    }
    let result = driver.run();
    if let Err(error) = &result {
        if let Err(capture_error) = driver.screenshot("failure") {
            eprintln!("Cannot capture failure screenshot: {capture_error}");
        }
        fs::write(driver.artifacts.join("failure.txt"), error.to_string())?;
    }
    let cleanup = driver.request("DELETE", "", None);
    result?;
    cleanup?;
    println!(
        "Core browser checks passed; artifacts: {}",
        driver.artifacts.display()
    );
    Ok(())
}
