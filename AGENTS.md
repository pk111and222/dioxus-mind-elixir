# 仓库实施约定

## 项目与维护

Rust + Dioxus 脑图组件，基于 Mind Elixir Core 重写，已发布到 crates.io。组件内可见文字一律经 `Labels` 由宿主提供，默认英文，不内置其他语言；非核心的界面功能交给宿主实现。
英文使用文档在 README.md，中文在 README_ZH.md；功能、依赖和平台说明需同步维护。
公开演示在 demo，scripts/build-pages 生成 GitHub Pages 静态资源。
合并到 main 即自动发版（.github/workflows/release.yml）：分支改了 Cargo.toml 版本就用该版本，否则小版本加 1；发版提交与标签推到 release 分支，main 不改写。改动记在 CHANGELOG.md 的 “## X.Y.Z — unreleased” 下。
保留独立 core/plugins 测试工程和 Rust 浏览器回归。平台边界见 docs/compatibility.md，来源见 NOTICE.md。
项目全部由 AI 生成并做过人工回归；接受 AI 贡献，无需额外标注。

## 样式

- 尽可能将可定制视觉值暴露为有默认值的公开 CSS 变量，采用 `--dme-` 命名并维护变量文档。
- 支持宿主通过外层容器、组件 class/style 和外部样式表覆盖内层样式；默认主题不能截断变量继承或依靠高优先级固定值阻止定制。
- 覆盖节点、连线、状态、编辑层、菜单、工具栏、大纲和拖放提示，保证不同实例相互隔离。
- 改变几何相关变量时重新测量并更新连线；Rust 几何计算使用实际生效的样式与尺寸，避免另一份硬编码常量。
- 核心和插件分别提供外部 CSS 定制示例与相应检查。

## Rust 实现

- 核心、插件、导出、示例和项目维护的测试代码使用 Rust。不保留上游 TS/JS，不编写内嵌 JS、`document::eval` 或 npm 插件包装实现。
- 优先使用 Dioxus Rust API；Web 可通过 `web-sys` / `wasm-bindgen` 的 Rust 绑定访问平台能力。Desktop 遇到能力缺口先寻找 Rust 接口或扩展路径，不自行退回 JS。
- Dioxus、WebView 和 WASM 工具链自身的运行支持与自动生成绑定属于依赖实现；不要将其误报为项目手写代码，也不要宣称浏览器产物不存在任何 JavaScript。
- 官方插件是行为与格式参考，对应能力用 Rust 重写，不加载原版编辑器或官方 npm 插件运行时。
- 保留数据与主要交互契约；内部实现允许为类型、所有权和可维护性大胆重写，显式记录行为差异。用户要求与本约定优先于历史设计建议。

## 错误处理与惯用写法

- 使用 enum/struct/trait 与所有权表达状态；可预期失败通过有语义的错误及 `Result` / `Option` 处理，优先 `?`、模式匹配和显式校验。
- 生产代码不使用 `unwrap()`、`expect()`、`panic!()`、`todo!()`、`unimplemented!()` 处理失败。不以默认值掩盖无效数据或未实现能力。
- 同时检查索引、算术、动态借用和异步生命周期的可达 panic；避免跨 await 持有可变状态借用，不静默吞掉异步错误。
- 自行编写的 Rust 代码默认 `forbid(unsafe_code)`；初始化时落实 `clippy::unwrap_used`、`expect_used` 和 `panic` 约束。
- 测试优先返回 `Result`，可使用断言；固定夹具确需 `expect` 时仅在测试作用域局部说明，不给生产代码放行。
- 无效输入、目标失效、取消、平台失败和卸载后的异步结果须有测试，失败后保持文档、索引、选择和历史一致。
