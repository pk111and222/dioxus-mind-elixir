# Compatibility and platform support

This page describes the `0.2` library. [English usage](../README.md) · [中文使用说明](../README_ZH.md)

## Data and state

The model is a strict, nested, **plain-text subset** of Mind Elixir JSON. Supported document fields: `nodeData`, `direction`, `compact`, `meta`. Node fields: `id`, `topic`, `children`, `expanded`, `direction`, `style`, `branchColor`, `note`, `metadata`. Style fields: `fontSize`, `fontFamily`, `color`, `background`, `fontWeight`, `width`, `border`, `textDecoration`.

Document directions are `0` left, `1` right, `2` both sides and `3` down. IDs must be nonempty and unique per document. Notes and arbitrary metadata are preserved, including explicit JSON null; notes are not rendered. Unknown fields are rejected with a typed error, rather than silently discarded. Images, rich HTML, tags, icons, hyperlinks, arrows, summaries and upstream JSON themes are unsupported. Topics render as text.

Checked operations commit atomically and share selection and bounded snapshot history. IDs address collapsed descendants. Multiple views share a `MindMapHandle`; viewport and outline browsing stay local to each view. Core mutations work without plugins. Keep plugin/subscription guards alive, and use `Result` to handle invalid inputs, read-only edits and stale operations. Callbacks execute after state borrows are released.

## Editing and gestures

The editor uses a native textarea. Double-click/F2 edits; Enter/Tab commits; Shift+Enter inserts a newline; Escape or blur cancels. Blank or unchanged drafts do not add history. Composition events protect editing keys; actual OS IME candidate confirmation remains a manual platform check.

Built-in plugins add structural shortcuts, spatial navigation, multi-selection, Shift-drag marquee, pointer drag/drop, edge pan, context commands, toolbar and shared outline. No npm plugin or upstream runtime is loaded. The Rust plugin API is not a JavaScript plugin compatibility layer. Host code can enable/disable or unregister capabilities per document.

## Export boundaries

| Format | Behavior |
| --- | --- |
| JSON | Complete persistence format for the supported model, including IDs, layout, collapse and metadata. |
| Plaintext | Two-space indented `- Topic` trees, escaping backslash/newline; preserves labels and order, not full document state. Unsupported rich fields or syntax return an error. |
| XMind | Independent modern ZIP/JSON encoder; includes the entire tree, IDs, notes, collapse, layout and supported text styling. Arbitrary CSS/metadata is not a lossless XMind backup. Import is not provided. |
| SVG | Web live capture with measured geometry, resolved CSS and embedded accessible fonts. Pan/zoom, selection and editor controls are excluded. |
| PNG/JPEG/WebP | Web canvas rasterization of the capture; validates the actual output MIME. Browser encoder, dimensions and memory limits apply. |
| HTML | Offline **static** view of a trusted SVG capture; no interactive editor or scripts. |

Web capture requires this library's `web` feature and `wasm32-unknown-unknown`. Capture a mounted, idle map, outside editing/animation. Cross-origin fonts/CSS resources must be readable or embedded; failures return errors. Only pass SVG produced by a trusted capture to `static_html`.

Desktop live SVG/image/HTML capture is **unsupported**: Dioxus 0.7.10's safe Rust renderer APIs do not expose the required computed-style/DOM snapshot. Capture returns a recoverable error. Tree operations, JSON, XMind encoding and host-owned file sinks work on Desktop. There is no JavaScript/eval fallback or estimated image pretending to reflect live CSS.

## Validation

The project is entirely AI-generated and has undergone human regression testing. Chromium browser regressions, core/plugin contract tests, Rust 1.85 checks, release Web/macOS builds, and an independent crates.io consumer have passed locally. macOS Desktop has had runtime/manual checks. Regression testing does not establish complete upstream compatibility.

Safari, Windows/Linux Desktop runtime, real OS IME candidate confirmation, and opening exports in the commercial XMind application remain unverified. XMind output has been independently parsed as ZIP/JSON. The transitive Desktop dependency `block 0.1.6` emits a Rust future-incompatibility warning. Remote GitHub workflows must run after the repository is pushed; local checks do not establish a successful remote run.
