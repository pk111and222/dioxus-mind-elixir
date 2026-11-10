# Tests / 测试

All regression code lives here. The public example is [`../demo/`](../demo/).
Core and plugin tests remain independent Cargo packages; neither needs the other.
测试集中在本目录；核心与插件仍是独立可运行的 Cargo 包，不互相依赖。

| Path | Purpose / 用途 |
| --- | --- |
| `foundation.rs` | Published library integration tests / 发布库的集成测试 |
| `core/src`, `core/tests` | Core fixtures, contract tests and visual regression app / 核心夹具、契约测试及可视化回归应用 |
| `core/browser` | Rust WebDriver runner for the core app / 核心应用的浏览器驱动 |
| `plugins/src`, `plugins/tests` | Plugin fixtures, contracts, lifetimes and visual regression app / 插件夹具、契约、生命周期及可视化回归应用 |
| `plugins/browser` | Rust WebDriver runner for the plugin app / 插件应用的浏览器驱动 |

## Contract tests / 契约测试

From the repository root / 仓库根目录：

```sh
cargo test -p dioxus-mind-elixir --locked
cargo test -p mind-elixir-core-tests --locked
cargo test -p mind-elixir-plugin-tests --locked
# Optional performance baseline / 可选性能基线
cargo test -p mind-elixir-core-tests --test performance -- --ignored --nocapture
```

Only `foundation.rs` is a root integration-test target. The subdirectories are
workspace packages, not additional root test targets. All test packages set
`publish = false`; their sources and font fixtures are excluded from the crate.

只有 `foundation.rs` 是根库集成测试；子目录是 workspace 测试包，不重复注册为根库测试。
所有测试包均禁止发布，测试应用与字体夹具不包含在 crate 中。

## Visual apps / 可视化回归

Requires Dioxus CLI 0.7.10 and the selected platform's dependencies.
分别运行，每次选择一个 renderer：

```sh
cd tests/core
dx serve --platform web --port 8088
# Or / 或：dx serve --platform desktop
```

For plugins, use `cd tests/plugins` from the repository root, port `8089`.
插件应用从根目录进入 `tests/plugins`，Web 端口使用 `8089`。

Core covers layouts, connectors, external CSS, editing, history, shared views,
read-only behavior and unmounting. Plugins cover commands, selection, drag/drop,
outline, locale, CSS and exports. Desktop live image/HTML capture returns the
documented platform error. Desktop file checks write to `target/plugin-artifacts`.

核心覆盖布局、连线、外部 CSS、编辑、历史、共享视图、只读和卸载；插件覆盖命令、选择、
拖放、大纲、语言、CSS 与导出。Desktop 实时图片/HTML 捕获返回已注明的平台错误。
Desktop 文件检查写入 `target/plugin-artifacts`。

## Browser regressions / 浏览器回归

Requires Chrome and a matching ChromeDriver, Rust, Dioxus CLI 0.7.10 and the
`wasm32-unknown-unknown` target. Run ChromeDriver separately, then invoke the
scripts from the repository root:

```sh
chromedriver --port=9515
# In another terminal / 另一个终端
scripts/test-core-e2e
scripts/test-plugins-e2e
```

The scripts start and clean up the matching visual app. The Rust runners use W3C
WebDriver without execute-script or handwritten JS/TS. Connect to an existing app
by setting `DME_APP_URL`; other options are `DME_WEBDRIVER_URL`, `DME_BROWSER`
and `DME_E2E_ARTIFACTS`. Set `DME_CHROME_BINARY` to select the Chrome executable
that matches ChromeDriver (CI uses the setup action output). Results, screenshots and actual exported files are written
to `target/core-e2e` or `target/plugins-e2e`; failures return a nonzero exit code.

脚本启动并清理对应测试应用；Rust 驱动不执行手写 JS/TS。设置 `DME_APP_URL` 可连接已有应用，
其余选项为 `DME_WEBDRIVER_URL`、`DME_BROWSER`、`DME_E2E_ARTIFACTS`。
结果、截图与实际导出文件写入 `target/core-e2e` 或 `target/plugins-e2e`；失败返回非零状态。

Safari requires the user to enable remote automation first; use `safaridriver`
with `DME_BROWSER=safari` and its matching WebDriver URL. Real OS input-method
candidate confirmation and cross-platform runtime checks remain manual;
see [compatibility](../docs/compatibility.md).

The plugin fixture Abel-Regular.ttf is from [Google Fonts Abel](https://github.com/google/fonts/tree/main/ofl/abel),
by MADType, under SIL OFL 1.1. Its license is retained in `plugins/fixtures/OFL-Abel.txt`.
