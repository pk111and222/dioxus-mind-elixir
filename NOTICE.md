# Attribution

This project is a Rust + Dioxus component version and fork of Mind Elixir Core:
https://github.com/ssshooter/mind-elixir-core
Official website: https://mind-elixir.com/

The upstream is MIT licensed,
Copyright (c) 2019 DjZhou.

Source baseline: https://github.com/ssshooter/mind-elixir-core/tree/73de5c6a617a95fe649cd8e0df7d8ca44b446f61
Package version at that baseline: 6.0.0.

The nested HTML/CSS tree rendering approach and operation behavior inform this
project. The CSS renderer and transactional state are Rust-native rewrites. Main-branch
quadratic and rounded vertical connector formulas follow the pinned upstream
source; horizontal sub-branches use measured edge-center cubic connectors.
See docs/compatibility.md for supported fields and intentional behavior differences.
The complete upstream license is distributed in LICENSE-MIND-ELIXIR. New project
code is covered by LICENSE. No upstream TypeScript/JavaScript runtime is bundled.

No vendored Viselect or official export-plugin code is included.
Official export sources were audited at the pinned commits below.
Export encoders are independent implementations of file formats; no
upstream export templates, theme data, icon assets, or plugin source are included.
The independent plugin test app includes Abel-Regular.ttf from Google Fonts under
SIL OFL 1.1, with its complete license in tests/plugins/fixtures/OFL-Abel.txt.
This test font is not included in the published library package.

## Official plugin references

These pinned sources were used for behavior/format research. Their scripts, templates, themes and icons were not copied or loaded. There is no binary/API compatibility claim.

| Reference | Version / commit | Scope and license findings |
| --- | --- | --- |
| [export-mindmap](https://github.com/mind-elixir/plugins/tree/81892c6777c7ae1e9beeec523abdb326c78e9e11/packages/export-mindmap) | 0.1.11 / `81892c6777c7ae1e9beeec523abdb326c78e9e11` | Package declares MIT, archive did not include LICENSE. Reference for resolving DOM styles and SVG foreignObject/canvas pipeline. Rust supports live SVG/PNG/JPEG/WebP. |
| [export-xmind](https://github.com/ssshooter/export-xmind/tree/6690e97e1b081dcaa8932d20b4f14ddd58fc678c) | 2.0.1 / `6690e97e1b081dcaa8932d20b4f14ddd58fc678c` | No license declaration found in repository/package. Modern XMind ZIP/JSON implemented independently; no source, themes or templates copied. |
| [export-html](https://github.com/ssshooter/export-html/tree/a3ed69102d7f8f2d3f1a22a809e78aed8c2237ab) | 3.0.0 / `a3ed69102d7f8f2d3f1a22a809e78aed8c2237ab` | No license declaration found. Independent static HTML/CSS/SVG generation; no interactive upstream editor. |

Other upstream plugins, including XMind/Markdown/FreeMind imports and open-desktop, are not included in the release.
