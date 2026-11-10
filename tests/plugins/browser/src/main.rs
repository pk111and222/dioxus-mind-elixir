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
            let mut options = json!({"args":["--headless=new","--window-size=1600,1100","--disable-gpu","--no-sandbox"]});
            if let Ok(binary) = std::env::var("DME_CHROME_BINARY") {
                options["binary"] = json!(binary);
            }
            json!({"browserName":"chrome","goog:chromeOptions":options})
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
            std::env::var("DME_E2E_ARTIFACTS").unwrap_or_else(|_| "target/plugins-e2e".into()),
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
    fn chord(&self, selector: &str, modifier: &str, key: &str) -> Result {
        self.click(selector)?;
        self.request("POST","/actions",Some(json!({"actions":[{"type":"key","id":"keyboard","actions":[{"type":"keyDown","value":modifier},{"type":"keyDown","value":key},{"type":"keyUp","value":key},{"type":"keyUp","value":modifier}]}]})))?;
        Ok(())
    }
    fn drag(&self, source: &str, target: &str) -> Result {
        let a = self.rect(source)?;
        let b = self.rect(target)?;
        self.request("POST","/actions",Some(json!({"actions":[{"type":"pointer","id":"mouse","parameters":{"pointerType":"mouse"},"actions":[
            {"type":"pointerMove","duration":50,"x":(a.x+a.width/2.0) as i64,"y":(a.y+a.height/2.0) as i64},
            {"type":"pointerDown","button":0},{"type":"pointerMove","duration":500,"x":(b.x+b.width/2.0) as i64,"y":(b.y+b.height/2.0) as i64},{"type":"pointerUp","button":0}
        ]}]})))?;
        Ok(())
    }
    fn document(&self) -> Result<Value> {
        Ok(serde_json::from_str(&self.text("#document")?)?)
    }
    fn run(&self) -> Result {
        let url = std::env::var("DME_APP_URL").unwrap_or_else(|_| "http://localhost:8089".into());
        self.request("POST", "/url", Some(json!({"url":url})))?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 8))?;
        self.click("details summary")?;
        self.screenshot("plugins-initial")?;
        self.keys("#map [data-node-id='design']", "\u{e007}")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 9))?;
        self.button("Undo")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 8))?;
        self.chord("#map [data-node-id='design']", "\u{e008}", "\u{e007}")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 9))?;
        self.button("Undo")?;
        self.chord("#map [data-node-id='design']", "\u{e009}", "\u{e007}")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 9))?;
        self.button("Undo")?;
        self.keys("#map [data-node-id='design']", "\u{e004}")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 9))?;
        self.button("Undo")?;
        self.click("#keyboard")?;
        self.keys("#map [data-node-id='design']", "\u{e007}")?;
        thread::sleep(Duration::from_millis(250));
        ensure(
            self.elements("#map [data-node-id]")?.len() == 8,
            "Disabled keyboard mutated document",
        )?;
        self.click("#keyboard")?;
        println!("shortcuts passed");
        // Outline edits a collapsed descendant and shares map undo history.
        self.click("#map [data-node-id='design'] .dme-collapse")?;
        self.keys("[data-outline-id='css']", "\u{e032}")?;
        self.wait(|| Ok(!self.elements(".dme-outline-editor")?.is_empty()))?;
        self.keys(".dme-outline-editor", "\u{e009}a\u{e000}大纲修改\u{e007}")?;
        self.wait(|| Ok(self.text("[data-outline-id='css']")?.contains("大纲修改")))?;
        ensure(
            self.elements("#map [data-node-id='css']")?.is_empty(),
            "Outline changed map collapse",
        )?;
        self.button("Undo")?;
        self.button("Undo")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 8))?;
        self.click("#map [data-node-id='design']")?;

        println!("outline passed");
        // Ctrl+click selection through a native pointer action.
        self.click("#map [data-node-id='design']")?;
        let r = self.rect("#map [data-node-id='build']")?;
        self.request("POST","/actions",Some(json!({"actions":[
            {"type":"key","id":"key","actions":[{"type":"keyDown","value":"\u{e008}"},{"type":"pause","duration":100},{"type":"pause","duration":100},{"type":"keyUp","value":"\u{e008}"}]},
            {"type":"pointer","id":"pointer","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerMove","x":(r.x+r.width/2.0) as i64,"y":(r.y+r.height/2.0) as i64},{"type":"pointerDown","button":0},{"type":"pointerUp","button":0}]}
        ]})))?;
        self.wait(|| Ok(self.elements("#map [data-selected='true']")?.len() == 2))?;
        // Menu opens with native secondary click and closes with Escape.
        let r = self.rect("#map [data-node-id='build']")?;
        self.request("POST","/actions",Some(json!({"actions":[{"type":"pointer","id":"context","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerMove","x":(r.x+r.width/2.0) as i64,"y":(r.y+r.height/2.0) as i64},{"type":"pointerDown","button":2},{"type":"pointerUp","button":2}]}]})))?;
        self.wait(|| Ok(!self.elements(".dme-context-menu")?.is_empty()))?;
        self.screenshot("context-menu")?;
        self.click(".dme-context-menu button")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 9))?;
        self.button("Undo")?;
        self.keys("#map [data-node-id='build']", "\u{e00c}")?;
        self.wait(|| Ok(self.elements(".dme-context-menu")?.is_empty()))?;
        // Marquee uses a Shift + blank-canvas drag; no document/history write.
        let a = self.rect("#map [data-node-id='design']")?;
        let b = self.rect("#map [data-node-id='release']")?;
        let before = self.document()?;
        self.request("POST","/actions",Some(json!({"actions":[
            {"type":"key","id":"marquee-key","actions":[{"type":"keyDown","value":"\u{e008}"},{"type":"pause","duration":10},{"type":"pause","duration":10},{"type":"pause","duration":500},{"type":"pause","duration":10},{"type":"keyUp","value":"\u{e008}"}]},
            {"type":"pointer","id":"marquee-mouse","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerMove","x":(a.x-15.0) as i64,"y":(a.y-15.0) as i64},{"type":"pointerDown","button":0},{"type":"pointerMove","duration":500,"x":(b.x+b.width+15.0) as i64,"y":(b.y+b.height+15.0) as i64},{"type":"pause","duration":50},{"type":"pointerUp","button":0},{"type":"pause","duration":10}]}
        ]})))?;
        self.wait(|| Ok(self.elements("#map [data-selected='true']")?.len() >= 3))?;
        ensure(self.document()? == before, "Marquee wrote document history")?;
        println!("multiselect passed");
        self.click("#reset")?;
        for layout in ["LEFT", "RIGHT", "SIDE", "DOWN"] {
            self.click(&format!("#layout-{layout}"))?;
            self.wait(|| {
                Ok(self.attribute("#map .dme-map", "data-layout")?.as_str()
                    == Some(&layout.to_lowercase()))
            })?;
            thread::sleep(Duration::from_millis(350));
            self.drag(
                "#map [data-node-id='release']",
                "#map [data-node-id='build']",
            )?;
            self.wait(|| {
                Ok(self
                    .document()?
                    .pointer("/nodeData/children/1/children/2/id")
                    .and_then(Value::as_str)
                    == Some("release"))
            })?;
            self.button("Undo")?;
        }
        // Escape after crossing the threshold must cancel without a delayed move.
        let before = self.document()?;
        let a = self.rect("#map [data-node-id='release']")?;
        let b = self.rect("#map [data-node-id='build']")?;
        self.request("POST","/actions",Some(json!({"actions":[
            {"type":"pointer","id":"cancel-mouse","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerMove","x":(a.x+a.width/2.0) as i64,"y":(a.y+a.height/2.0) as i64},{"type":"pointerDown","button":0},{"type":"pointerMove","duration":300,"x":(b.x+b.width/2.0) as i64,"y":(b.y+b.height/2.0) as i64}]}
        ]})))?;
        self.wait(|| Ok(!self.elements(".dme-drag-status")?.is_empty()))?;
        self.request("POST","/actions",Some(json!({"actions":[{"type":"key","id":"cancel-key","actions":[{"type":"keyDown","value":"\u{e00c}"},{"type":"keyUp","value":"\u{e00c}"}]}]})))?;
        self.request("DELETE", "/actions", None)?;
        self.wait(|| Ok(self.elements(".dme-drag-status")?.is_empty()))?;
        ensure(
            self.document()? == before,
            "Cancelled drag mutated document",
        )?;
        // Holding a drag stationary in the edge band keeps panning until cancel.
        let a = self.rect("#map [data-node-id='release']")?;
        let bounds = self.rect("#map .dme-map")?;
        self.request("POST","/actions",Some(json!({"actions":[{"type":"pointer","id":"edge-mouse","parameters":{"pointerType":"mouse"},"actions":[{"type":"pointerMove","x":(a.x+a.width/2.0) as i64,"y":(a.y+a.height/2.0) as i64},{"type":"pointerDown","button":0},{"type":"pointerMove","duration":200,"x":(bounds.x+bounds.width-8.0) as i64,"y":(bounds.y+bounds.height/2.0) as i64}]}]})))?;
        self.wait(|| Ok(!self.elements(".dme-drag-status")?.is_empty()))?;
        let transform = self.css("#map .dme-canvas", "transform")?;
        thread::sleep(Duration::from_millis(180));
        ensure(
            self.css("#map .dme-canvas", "transform")? != transform,
            "Stationary edge drag did not continue panning",
        )?;
        self.request("POST","/actions",Some(json!({"actions":[{"type":"key","id":"edge-cancel","actions":[{"type":"keyDown","value":"\u{e00c}"},{"type":"keyUp","value":"\u{e00c}"}]}]})))?;
        self.request("DELETE", "/actions", None)?;
        self.keys("#map .dme-map", "\u{e011}")?;
        println!("drag passed");
        self.click("#layout-RIGHT")?;
        self.click("#custom")?;
        // CSS-only outline indentation and collapse are scoped to this view.
        self.click("[data-outline-id='design'] .dme-outline-fold")?;
        self.wait(|| Ok(self.elements("[data-outline-id='css']")?.is_empty()))?;
        ensure(
            !self.elements("#map [data-node-id='css']")?.is_empty(),
            "Outline collapse changed map",
        )?;
        self.click("[data-outline-id='design'] .dme-outline-fold")?;
        self.wait(|| {
            Ok(self.css("#map [data-node-id='css']", "font-size")?.as_str() == Some("18px"))
        })?;
        ensure(
            self.css(".dme-toolbar", "background-color")?.as_str()
                == Some("rgba(237, 231, 249, 1)"),
            "Toolbar lost inherited CSS variables",
        )?;
        self.screenshot("plugins-custom")?;
        self.click("#font")?;
        thread::sleep(Duration::from_millis(350));
        self.click("#export-images")?;
        self.wait(|| {
            Ok(self.text("#export-status")?.contains("ready")
                || self.text("#export-status")?.contains("failed"))
        })?;
        ensure(self.text("#error")?.is_empty(), &self.text("#error")?)?;
        for (name, mime) in [
            ("mind-map.svg", "image/svg+xml"),
            ("mind-map.html", "text/html"),
            ("mind-map.png", "image/png"),
            ("mind-map.jpg", "image/jpeg"),
            ("mind-map.webp", "image/webp"),
        ] {
            let selector = format!("[data-file='{name}'] a");
            let value = self.attribute(&selector, "href")?;
            let href = value.as_str().ok_or("Missing file URL")?;
            let encoded = href
                .strip_prefix(&format!("data:{mime};base64,"))
                .ok_or("Bad export MIME")?;
            let bytes = base64::engine::general_purpose::STANDARD.decode(encoded)?;
            if name.ends_with(".svg") {
                let text = std::str::from_utf8(&bytes)?;
                ensure(
                    text.contains("data:font/ttf;base64,")
                        && text.contains("18px")
                        && text.contains("CSS 变量")
                        && text.contains("<path"),
                    "Export lost actual CSS, Unicode or links",
                )?;
            }
            fs::write(self.artifacts.join(name), bytes)?;
        }
        self.screenshot("plugins-exports")?;
        self.click("#readonly")?;
        let before = self.document()?;
        self.keys("#map [data-node-id='design']", "\u{e007}")?;
        ensure(
            self.document()? == before,
            "Readonly shortcut changed document",
        )?;
        self.click("#readonly")?;
        self.click("#language")?;
        self.button("撤销")?;
        self.click("#mount")?;
        self.click("#export-images")?;
        self.wait(|| Ok(self.text("#error")?.contains("unmounted")))?;
        self.click("#mount")?;
        self.click("#reset")?;
        self.wait(|| Ok(self.elements("#map [data-node-id]")?.len() == 8))?;
        // Offline exported HTML must render without editor scripts or host assets.
        let html = fs::read(self.artifacts.join("mind-map.html"))?;
        self.request("POST","/url",Some(json!({"url":format!("data:text/html;base64,{}",base64::engine::general_purpose::STANDARD.encode(html))})))?;
        self.wait(|| Ok(self.elements(".dme-topic")?.len() == 8))?;
        ensure(
            self.elements("script")?.is_empty(),
            "Static HTML contains scripts",
        )?;
        self.screenshot("standalone-html")?;
        fs::write(
            self.artifacts.join("result.txt"),
            "PASS: native context-menu command, marquee, Escape cancellation, stationary edge pan, plugin shortcuts, enable/disable, outline shared editing/history/collapsed IDs, multiselect, four-layout pointer drag, external CSS, all Web image formats, static offline HTML, readonly, unmount errors.\n",
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
                std::env::var("DME_E2E_ARTIFACTS").unwrap_or_else(|_| "target/plugins-e2e".into()),
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
        "Plugin browser checks passed; artifacts: {}",
        driver.artifacts.display()
    );
    Ok(())
}
