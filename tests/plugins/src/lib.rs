//! Fixtures owned by the independent plugin test application.
use dioxus_mind_elixir::{MindMapData, Node};
/// Small Unicode tree with editable, collapsible and draggable branches.
pub fn fixture() -> MindMapData {
    MindMapData::new(Node::new("root", "Rust 插件工作台").with_children(vec![
        Node::new("design", "设计 Design").with_children(vec![
            Node::new("css", "CSS 变量"),
            Node::new("api", "插件 API"),
        ]),
        Node::new("build", "构建 Build").with_children(vec![
            Node::new("tests", "独立测试"),
            Node::new("export", "导出文件"),
        ]),
        Node::new("release", "验收 Release"),
    ]))
}
