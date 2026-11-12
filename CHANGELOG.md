# Changelog

## 0.2.0 — 2026-11-12

- **Breaking:** `Locale` is replaced by `plugins::Labels`, a struct of every text the components show
  (commands, accessible names, hints, the topic of new nodes). It defaults to English; hosts pass
  `labels` to `EditableMindMap`, `Outline` and `Toolbar` to change any of them. The component no
  longer ships Chinese text. `plugins::execute` and `plugins::command_operation` take the topic of
  new nodes instead of a locale.
- Fixed: editing a node and then clicking elsewhere failed with "The document changed during the
  operation" (the click's selection change invalidated the draft). Only changes to the edited node
  itself now close the editor; the outline editor behaves the same way.
- The inline topic editor sits in the node and grows with its text (transparent, no border or resize
  handle by default); `--dme-editor-width`, `--dme-editor-caret` and `--dme-editing-outline`.
- Branches expose their effective color as `--dme-branch-color` (the node's `branch_color`, else the
  theme palette color of its main branch).

## 0.1.1 — unreleased

- Node pictures: `Node.image` (`NodeImage { url, width, height, alt }`), rendered above the topic with a
  host `image_url` resolver, a visible fallback when loading fails, and `Operation::SetImage`.
- `Operation::SetNote` edits plain-text notes as undoable transactions; nodes with notes or pictures show
  small markers on the canvas and in the outline.
- Host-visible topic drafts: `MindMapHandle::{pending_edit, commit_pending, cancel_pending}` and
  `PendingEdit`, shared by the map and outline editors. `blur_commits` on `EditableMindMap`/`Outline`
  commits instead of discarding a draft on blur (IME composition is still protected).
- `fit_revision` fits all visible nodes into the view (never magnifies); `plugins::gesture::fit`.
- JSON documents may now contain `image`; malformed pictures are rejected with a path.

- Consolidate the public demo under `demo/` and all regression applications/runners under `tests/`; keep two reference documents.
- Replace implementation-stage notes and the initialization example with maintained usage, compatibility and provenance documents.
- Add English and Chinese READMEs with Cargo setup, dependency versions and the project's AI contribution policy.
- Add a bilingual Rust/Dioxus playground, static Pages build script and GitHub Pages workflow.

## 0.1.0 — 2026-11-10

First Rust-native Dioxus implementation, published to crates.io.

- Strict plain-text JSON model, four HTML/CSS layouts, measured SVG connectors.
- Checked transactional tree operations, selection, snapshot undo/redo, readonly and before hooks.
- Shared map/outline editing, structural shortcuts, multi-selection, marquee, drag/drop, toolbar and menus.
- Typed per-document plugin registration and subscription lifetimes, English/Chinese labels.
- Inheritable `--dme-*` CSS variables, themes and per-view zoom/pan.
- Plaintext tree conversion and modern XMind ZIP export.
- Web live CSS/font capture to SVG, PNG, JPEG, WebP and offline static HTML.
- Independent Rust core/plugin test applications and WebDriver regression runners.

Limits: Desktop live image/HTML capture is unsupported. Safari, real OS IME candidate
confirmation, Windows/Linux Desktop runtime and target XMind application checks
remain unverified. JSON is the complete supported persistence format; plaintext
and XMind exports have explicitly documented format limits. HTML is static.

No upstream JavaScript/TypeScript plugin runtime is bundled. Dioxus and the WebView/
WASM toolchain supply their own runtime and generated bindings. The library and
project code forbid unsafe Rust and deny unwrap/expect/panic in production checks.
