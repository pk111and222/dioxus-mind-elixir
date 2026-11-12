# dioxus-mind-elixir

[English](README.md) · [简体中文](README_ZH.md) · [crates.io](https://crates.io/crates/dioxus-mind-elixir) · [API reference](https://docs.rs/dioxus-mind-elixir/0.1.0/dioxus_mind_elixir/)

A **Rust + Dioxus component version and fork of [Mind Elixir Core](https://github.com/ssshooter/mind-elixir-core)**, rewritten in Rust from its data model and interaction design. Visit the [original Mind Elixir website](https://mind-elixir.com/). Thank you to its authors for the foundation.

**AI policy:** This project is entirely AI-generated and has undergone human regression testing. If you object to AI-generated software, please do not use this project. AI-generated merge requests (MRs/PRs) are welcome; no AI disclosure label is required. Contributions still need to meet the project's quality and testing requirements.

## What it provides

- Editable plain-text mind maps with left, right, both-side and downward layouts.
- Checked tree operations, shared document/selection, atomic edits and undo/redo.
- Keyboard commands, multi-selection, marquee, drag/drop, context menu and toolbar through optional Rust plugins.
- An outline that shares the map's document and history, with English/Chinese command labels.
- Inheritable `--dme-*` CSS variables for nodes, connectors, editor and plugins.
- Strict JSON persistence, plaintext conversion and modern XMind export.
- Web live CSS/font capture to SVG, PNG, JPEG, WebP and static HTML.

This is a Rust rewrite, not a drop-in implementation of every upstream API or plugin. Unknown JSON fields are rejected. See [compatibility and platform limits](docs/compatibility.md).

## Requirements

| Dependency | Version / requirement |
| --- | --- |
| Rust | **1.85+**, edition 2024 |
| `dioxus-mind-elixir` | **0.1.0** on crates.io |
| Dioxus | **0.7.10**, matching the version used to build/test this release |
| Dioxus CLI (`dx`) | **0.7.10** for running/building an application |
| Web target | `wasm32-unknown-unknown` |
| Desktop | Dioxus `desktop` renderer and your platform's [native prerequisites](https://dioxuslabs.com/learn/0.7/getting_started/) |

No npm packages or upstream JavaScript plugins are needed. Project-maintained functionality is Rust. Dioxus and the WebView/WASM toolchain provide their own runtime and generated JavaScript bindings.

## Install from Cargo: Web

Create an application and add the published crate:

```sh
cargo new my-mind-map
cd my-mind-map
cargo add dioxus@=0.7.10 --no-default-features --features macro,html,signals,hooks,mounted,launch,web
cargo add dioxus-mind-elixir@0.1.0 --features web
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.10 --locked
```

Equivalent `Cargo.toml`:

```toml
[package]
name = "my-mind-map"
version = "0.1.0"
edition = "2024"
rust-version = "1.85"

[dependencies]
dioxus = { version = "=0.7.10", default-features = false, features = ["macro", "html", "signals", "hooks", "mounted", "launch", "web"] }
dioxus-mind-elixir = { version = "0.1.0", features = ["web"] }
```

Put this in `src/main.rs`:

```rust
use dioxus::prelude::*;
use dioxus_mind_elixir::{
    EditableMindMap, MindMapData, MindMapError, Node, Outline, Toolbar,
    plugins::{Labels, Plugin},
    use_mind_map, use_plugins,
};

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let handle = match use_mind_map(MindMapData::new(
        Node::new("root", "Ideas").with_children(vec![
            Node::new("rust", "Build with Rust"),
            Node::new("style", "Make it yours"),
        ]),
    )) {
        Ok(handle) => handle,
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let _plugins = match use_plugins(handle, Plugin::builtins()) {
        Ok(plugins) => plugins,
        Err(error) => return rsx! { p { role: "alert", "{error}" } },
    };
    let mut error = use_signal(String::new);
    rsx! {
        main {
            style: "--dme-root-background:#315c46;--dme-node-radius:8px;",
            Toolbar { handle, labels: Labels::default() }
            EditableMindMap {
                handle,
                       labels: Labels::default(),
                style: "height:500px;",
                on_error: move |value: MindMapError| error.set(value.to_string()),
            }
            Outline { handle }
            p { role: "alert", "{error}" }
        }
    }
}
```

Run `dx serve --platform web`. Styles are embedded in the crate; there is no asset-copy step. Keep the guard returned by `use_plugins` alive. The toolbar and outline appear when their capabilities are installed; omit plugins to use core editing alone. Every visible text (commands, accessible names, hints) comes from `Labels`, English by default; pass `labels` to `Toolbar`, `EditableMindMap` and `Outline` to replace any field or translate all of them. The component ships no other language.

### Desktop

Use the same component code, but choose the Desktop renderer instead of Web:

```toml
[dependencies]
dioxus = { version = "=0.7.10", default-features = false, features = ["macro", "html", "signals", "hooks", "mounted", "launch", "desktop"] }
dioxus-mind-elixir = "0.1.0"
```

Run `dx serve --platform desktop`. The library has no default renderer. Do not enable its `web` feature for a Desktop application. Desktop live image/HTML capture is currently unsupported and returns a recoverable error; tree editing, JSON, XMind and host-provided file saving remain available.

## Data, operations and plugins

`use_mind_map` creates a checked shared `MindMapHandle`. Pass it to multiple views to share document, selection and history. Viewports and outline browsing stay local to each view. For read-only rendering, use `MindMap` with a `ReadSignal<MindMapData>`; for non-UI operations, use `MindMapState`.

These operations belong in an event handler or service, not during a component render:

```rust
use dioxus_mind_elixir::{LayoutDirection, MindMapData, MindMapError, MindMapHandle, Operation};

fn rename_and_save(handle: MindMapHandle) -> Result<String, MindMapError> {
    handle.apply(Operation::SetTopic { id: "rust".into(), topic: "Rust + Dioxus".into() })?;
    handle.apply(Operation::SetLayout(LayoutDirection::Side))?;
    handle.document()?.to_json()
}

fn load_json(handle: MindMapHandle, json: &str) -> Result<(), MindMapError> {
    handle.refresh(MindMapData::from_json(json)?)
}
```

`handle.undo()`, `redo()`, `select()`, `set_read_only()` and `apply()` return `Result`. `refresh` replaces the document and resets history. Subscribe with `handle.subscribe()` and retain the returned guard. `Plugin::builtins()` enables the standard capabilities; `register_plugin`, `set_plugin_enabled` and RAII guards control per-document registration and lifetime. No global registry or dynamic script loader is involved.

In the map: double-click/F2 edits, Enter adds a sibling, Tab adds a child, Ctrl/⌘+Z undoes, Ctrl/⌘+click selects multiple nodes and Shift+drag on blank space selects a region. While editing, Enter/Tab commits, Shift+Enter adds a newline and Escape cancels.

## Styling

Place variables on a common parent to style the map, toolbar and outline together:

```css
.my-map-host {
  --dme-root-background: #67518c;
  --dme-font-size: 18px;
  --dme-node-radius: 6px;
  --dme-gap-x: 64px;
  --dme-link-color: #b5a0d0;
  --dme-toolbar-background: #f3edf9;
  --dme-outline-background: #f3edf9;
}
```

Defaults preserve inheritance, so separate hosts can have independent themes. Explicit node styles or a `Theme` prop can override inherited values. Geometry uses measured DOM sizes; increase `measurement_revision` for external position-only changes. See the [complete CSS variable reference](docs/CSS变量.md) (Chinese, with CSS names and defaults).

## Export

`plugins::export::xmind(&handle.document()?)` returns an `ExportFile`. Use `file.save_with(...)` for a host-owned filesystem/dialog sink, or `file.download()` on Web. JSON is the complete persistence format; XMind and plaintext have documented field limits.

For Web images, retain `event.data()` from a wrapper's `onmounted` event as `Rc<MountedData>`, then call from an async event handler:

```rust
use dioxus::prelude::MountedData;
use dioxus_mind_elixir::{MindMapError, plugins::export};

async fn download_png(element: &MountedData) -> Result<(), MindMapError> {
    let options = export::ImageOptions::default();
    let svg = export::capture_svg(element, &options).await?;
    export::rasterize(&svg, export::ImageFormat::Png, &options).await?.download()
}
```

The example requires `web` and a WASM target. Capture an idle mounted map; readable/embedded fonts and resources are required. `static_html(title, &svg)` produces an offline static view, not an editable app. See [export and platform boundaries](docs/compatibility.md).

## Playground and GitHub Pages

The maintained public example lives in `demo`; core/plugin regression applications remain separate under `tests/`.

```sh
cd demo
dx serve --platform web
# Stop the preview, then build from the repository root:
cd ..
scripts/build-pages /dioxus-mind-elixir/
```

Deploy **`dist/pages/`** in full. Use `/` as the build argument for a custom domain or user site. The demo has English/Chinese UI, prominent Mind Elixir attribution, editing, shared outline, themes and downloads. `?lang=zh` opens Chinese; language switching preserves your document. Edits are in memory: download JSON before reloading.

A [GitHub Pages workflow](.github/workflows/pages.yml) is included. Select **GitHub Actions** under repository **Settings → Pages**. Every release deploys the playground from the new tag; run the workflow manually to redeploy (optionally from `release` or a tag). The workflow reads the hosting base path automatically. The bundle needs HTTP(S), not `file://`, to load WASM.

## Repository layout

```text
src/                 Component library
assets/              Embedded component stylesheet
demo/                The only public demo; also the GitHub Pages source
tests/
  foundation.rs      Library integration tests (Cargo convention)
  core/              Independent core regression application and tests
    browser/         Rust WebDriver runner for the core application
  plugins/           Independent plugin regression application and tests
    browser/         Rust WebDriver runner for the plugin application
docs/                CSS reference and compatibility limits
scripts/             Build and test entry points
```

`demo/` is for users; the two applications inside `tests/` are regression fixtures, not additional examples. Build output stays in ignored `target/` and `dist/` directories.

## Development

```sh
cargo fmt --all -- --check
cargo clippy -p dioxus-mind-elixir --all-targets --locked -- -D warnings
cargo test -p dioxus-mind-elixir --locked
cargo test -p mind-elixir-core-tests --locked
cargo test -p mind-elixir-plugin-tests --locked
cargo doc -p dioxus-mind-elixir --no-deps --locked
```

The independent core/plugin applications and their Rust browser runners are all under `tests/`. See the [testing guide](tests/README.md). Project code forbids unsafe Rust and denies production `unwrap`, `expect` and `panic` in Clippy checks.

### Releases

Merging into `main` releases automatically ([release workflow](.github/workflows/release.yml)):

1. CI runs in full; a failure stops the release.
2. The version is chosen by [`scripts/release-version`](scripts/release-version). If the merged branch changed `version` in `Cargo.toml`, that version is released. If `Cargo.toml` still names a version that has not been released, that version is used. Otherwise the patch of the latest release (tag or crates.io) is increased by one. The version must be newer than every released one.
3. One commit `release: vX.Y.Z` (version, `Cargo.lock`, dated `CHANGELOG.md` heading) is added on top of the merged commit, tagged `vX.Y.Z` and pushed to the **`release`** branch. `main` itself is never rewritten.
4. The tag is published to crates.io, a GitHub release is created from the matching `CHANGELOG.md` section, and the playground is deployed to Pages.

Write changes under `## X.Y.Z — unreleased` (or `## Unreleased`) in `CHANGELOG.md`. A run can be repeated safely: existing tags, crates.io versions and GitHub releases are reused. **Run workflow** accepts an explicit version. Repository setup: a `CARGO_REGISTRY_TOKEN` repository secret (a crates.io token with the publish-update scope) and **Settings → Actions → Workflow permissions** set to read and write. The workflow replaces the `release` branch on every release, so do not commit to it by hand.

Chromium regressions and macOS runtime/manual checks have been performed. Safari, Windows/Linux Desktop, real OS IME candidate confirmation and commercial XMind application checks remain unverified. See [compatibility](docs/compatibility.md).

## Attribution and license

MIT: [project license](LICENSE), [upstream license](LICENSE-MIND-ELIXIR), [NOTICE](NOTICE.md). Upstream source and plugin references are recorded in NOTICE. See the [changelog](CHANGELOG.md). This project is not an official Mind Elixir release.
