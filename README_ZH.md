# dioxus-mind-elixir

[English](README.md) · [简体中文](README_ZH.md) · [crates.io](https://crates.io/crates/dioxus-mind-elixir) · [API 文档](https://docs.rs/dioxus-mind-elixir/0.1.0/dioxus_mind_elixir/)

这是 **[Mind Elixir Core](https://github.com/ssshooter/mind-elixir-core) 的 Rust + Dioxus 组件版本，fork 自 Mind Elixir**，基于上游的数据模型与交互设计，以 Rust 重新实现。欢迎访问 [Mind Elixir 官网](https://mind-elixir.com/)，感谢上游作者提供的基础。

**AI 声明：本项目全部由 AI 生成，已做好人工回归测试。如果你对此有意见，请勿使用。MR/PR 接受 AI 代码，无需标注 AI 生成。** 贡献仍需满足项目的质量与测试要求。

## 功能

- 可编辑的纯文本脑图，支持向左、向右、左右和向下四种布局。
- 校验后的树操作、共享文档与选择、原子编辑、撤销和重做。
- 通过可选 Rust 插件提供快捷键、多选、框选、拖放、右键菜单与工具栏。
- 与地图共享文档、历史的大纲，内置中英文命令文案。
- 可继承的 `--dme-*` CSS 变量，覆盖节点、连线、编辑器和插件。
- 严格 JSON 持久化、纯文本转换与现代 XMind 导出。
- Web 实时 CSS/字体捕获，导出 SVG、PNG、JPEG、WebP 和静态 HTML。

本项目以 Rust 重写，不保证与上游全部 API 或插件直接兼容。未知 JSON 字段会报错，详见[兼容范围与平台限制](docs/compatibility.md)。

## 依赖版本

| 依赖 | 版本 / 要求 |
| --- | --- |
| Rust | **1.85+**，edition 2024 |
| `dioxus-mind-elixir` | crates.io **0.1.0** |
| Dioxus | **0.7.10**，与本版本构建和测试版本一致 |
| Dioxus CLI（`dx`） | **0.7.10**，运行与构建应用时使用 |
| Web target | `wasm32-unknown-unknown` |
| Desktop | Dioxus `desktop` renderer 与平台的[原生依赖](https://dioxuslabs.com/learn/0.7/getting_started/) |

无需 npm 包或上游 JavaScript 插件。项目维护的功能代码使用 Rust；Dioxus 与 WebView/WASM 工具链仍包含自身运行时和自动生成的 JavaScript 绑定。

## 从 Cargo 使用：Web

新建应用并安装已发布的 crate：

```sh
cargo new my-mind-map
cd my-mind-map
cargo add dioxus@=0.7.10 --no-default-features --features macro,html,signals,hooks,mounted,launch,web
cargo add dioxus-mind-elixir@0.1.0 --features web
rustup target add wasm32-unknown-unknown
cargo install dioxus-cli --version 0.7.10 --locked
```

等价的 `Cargo.toml`：

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

将以下内容放入 `src/main.rs`：

```rust
use dioxus::prelude::*;
use dioxus_mind_elixir::{
    EditableMindMap, MindMapData, MindMapError, Node, Outline, Toolbar,
    plugins::{Locale, Plugin},
    use_mind_map, use_plugins,
};

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    let handle = match use_mind_map(MindMapData::new(
        Node::new("root", "灵感").with_children(vec![
            Node::new("rust", "用 Rust 构建"),
            Node::new("style", "自由定制样式"),
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
            Toolbar { handle, locale: Locale::Zh }
            EditableMindMap {
                handle,
                locale: Locale::Zh,
                style: "height:500px;",
                on_error: move |value: MindMapError| error.set(value.to_string()),
            }
            Outline { handle, locale: Locale::Zh }
            p { role: "alert", "{error}" }
        }
    }
}
```

执行 `dx serve --platform web`。样式已内嵌，无需复制资源。必须保留 `use_plugins` 返回的 guard；工具栏、大纲需要对应插件能力，省略插件可只用核心编辑。`Locale::En` 提供英文命令文案。

### Desktop

组件代码相同，将依赖切换为 Desktop renderer：

```toml
[dependencies]
dioxus = { version = "=0.7.10", default-features = false, features = ["macro", "html", "signals", "hooks", "mounted", "launch", "desktop"] }
dioxus-mind-elixir = "0.1.0"
```

执行 `dx serve --platform desktop`。库默认不启用 renderer；Desktop 应用不要启用本库的 `web` feature。目前 Desktop 实时图片/HTML 捕获暂不支持，会返回可处理的错误；树编辑、JSON、XMind 与宿主文件保存接口可用。

## 数据、操作与插件

`use_mind_map` 创建校验后的 `MindMapHandle`。多个视图传入同一 handle 即可共享文档、选择与历史；视口和大纲浏览状态分别维护。只读渲染用 `MindMap` 与 `ReadSignal<MindMapData>`，无 UI 的操作用 `MindMapState`。

以下操作应放在事件回调或服务中，不要在组件渲染期间修改状态：

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

`handle.undo()`、`redo()`、`select()`、`set_read_only()`、`apply()` 均返回 `Result`。`refresh` 替换文档并重置历史。使用 `handle.subscribe()` 订阅并保留 guard。`Plugin::builtins()` 安装标准能力，`register_plugin`、`set_plugin_enabled` 与 RAII guard 控制每个文档的插件注册和生命周期；没有全局注册表或动态脚本加载器。

地图内双击/F2 编辑，Enter 添加同级，Tab 添加子级，Ctrl/⌘+Z 撤销，Ctrl/⌘+点击多选，Shift+拖动画布空白处框选。编辑时 Enter/Tab 提交、Shift+Enter 换行、Escape 取消。

## 样式定制

将变量放在共同父容器上，可同步影响地图、工具栏与大纲：

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

默认样式保留变量继承，不同父容器可使用独立主题。显式节点样式和 `Theme` prop 可覆盖继承值。几何计算读取真实 DOM 尺寸；外部只改变位置时，可增加 `measurement_revision` 主动重测。完整列表见 [CSS 变量文档](docs/CSS变量.md)。

## 导出

`plugins::export::xmind(&handle.document()?)` 返回 `ExportFile`。使用 `file.save_with(...)` 接入宿主文件系统/对话框，Web 可直接 `file.download()`。JSON 是完整持久化格式；XMind、纯文本有明确的字段边界。

Web 图片捕获需要在地图父容器 `onmounted` 时，将 `event.data()` 保存为 `Rc<MountedData>`，随后在异步事件处理器中使用：

```rust
use dioxus::prelude::MountedData;
use dioxus_mind_elixir::{MindMapError, plugins::export};

async fn download_png(element: &MountedData) -> Result<(), MindMapError> {
    let options = export::ImageOptions::default();
    let svg = export::capture_svg(element, &options).await?;
    export::rasterize(&svg, export::ImageFormat::Png, &options).await?.download()
}
```

上例需要 `web` feature 和 WASM target。捕获时地图需已挂载、未处于编辑或动画中，字体/资源必须可读取或内嵌。`static_html(title, &svg)` 输出离线静态页面，不包含可编辑应用。详见[导出与平台边界](docs/compatibility.md)。

## 双语演示与 GitHub Pages

正式演示在 `demo`；核心与插件回归应用独立保留在 `tests/`。

```sh
cd demo
dx serve --platform web
# 停止预览，回到仓库根目录构建静态资源：
cd ..
scripts/build-pages /dioxus-mind-elixir/
```

完整部署 **`dist/pages/`**。自定义域名或用户站点的构建参数用 `/`。页面包含中英文、显著的 Mind Elixir 来源及官网/GitHub 链接、编辑、共享大纲、主题与下载。`?lang=zh` 直接打开中文，切换语言保留文档。编辑保存在内存中，刷新前请下载 JSON。

已提供 [GitHub Pages 工作流](.github/workflows/pages.yml)。在仓库 **Settings → Pages** 选择 **GitHub Actions**，然后推送 `main` 或手动运行。工作流自动读取托管路径；默认分支不是 `main` 时需调整触发分支。静态页面需通过 HTTP(S) 访问，不能用 `file://` 加载 WASM。

## 仓库结构

```text
src/                 组件库源码
assets/              内嵌的组件样式
demo/                唯一的公开演示，同时作为 GitHub Pages 源码
tests/
  foundation.rs      库的集成测试（Cargo 标准目录约定）
  core/              独立核心回归应用和测试
    browser/         核心应用的 Rust WebDriver 驱动
  plugins/           独立插件回归应用和测试
    browser/         插件应用的 Rust WebDriver 驱动
docs/                CSS 变量参考、兼容与平台限制
scripts/             构建和测试入口
```

用户使用 `demo/`；`tests/` 里的两个应用是回归测试夹具，保留可视化入口便于定位问题。构建产物集中在已忽略的 `target/` 和 `dist/`，不属于源码。

## 开发检查

```sh
cargo fmt --all -- --check
cargo clippy -p dioxus-mind-elixir --all-targets --locked -- -D warnings
cargo test -p dioxus-mind-elixir --locked
cargo test -p mind-elixir-core-tests --locked
cargo test -p mind-elixir-plugin-tests --locked
cargo doc -p dioxus-mind-elixir --no-deps --locked
```

核心、插件的独立测试应用及 Rust 浏览器驱动统一放在 `tests/`，运行方式见[测试说明](tests/README.md)。项目禁止 unsafe Rust，生产代码的 Clippy 检查禁止 `unwrap`、`expect`、`panic`。

已做 Chromium 回归与 macOS 运行/人工检查；Safari、Windows/Linux Desktop、真实 OS 输入法候选确认与商业 XMind 应用打开检查仍待验证，详见[兼容说明](docs/compatibility.md)。

## 来源与许可

MIT：[本项目许可](LICENSE)、[上游许可](LICENSE-MIND-ELIXIR)、[来源声明](NOTICE.md)。上游源码与插件参考统一记录在来源声明中，另见[变更日志](CHANGELOG.md)。本项目不是 Mind Elixir 官方发行版。
