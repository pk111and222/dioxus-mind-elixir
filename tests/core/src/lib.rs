//! Public-API fixtures for the independent core verification package.
use dioxus_mind_elixir::{MindMapData, Node};

/// Multiline, Unicode and differently sized branches, without plugins.
pub fn fixture() -> MindMapData {
    MindMapData::new(Node::new("root", "Rust 核心脑图").with_children(vec![
        Node::new("design", "设计与表达").with_children(vec![
            Node::new("css", "CSS 变量\n外部定制"),
            Node::new(
                "long",
                "一段用于检查长文本和自动换行的节点内容 Rust and Dioxus",
            ),
        ]),
        Node::new("build", "构建与验证").with_children(vec![
            Node::new("web", "Web / WASM"),
            Node::new("desktop", "Desktop / WebView"),
        ]),
        Node::new("history", "历史与事务"),
    ]))
}
