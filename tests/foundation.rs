//! Foundation checks; independent stage-two/three suites are added later.
use dioxus_mind_elixir::{MindMapData, MindMapError, Node};

#[test]
fn ids_are_scoped_to_a_document() -> Result<(), MindMapError> {
    let first = MindMapData::new(Node::new("root", "中文\n多行"));
    let second = first.clone();
    first.validate()?;
    second.validate()?;
    Ok(())
}

#[test]
fn duplicate_and_empty_ids_are_rejected() {
    let duplicate =
        MindMapData::new(Node::new("root", "Root").with_children(vec![Node::new("root", "Child")]));
    assert_eq!(
        duplicate.validate(),
        Err(MindMapError::DuplicateNodeId("root".into()))
    );
    assert_eq!(
        MindMapData::new(Node::new("  ", "Root")).validate(),
        Err(MindMapError::EmptyNodeId)
    );
}

#[test]
fn excessive_depth_is_rejected_before_recursive_rendering() {
    let mut node = Node::new("leaf", "Leaf");
    for depth in 0..130 {
        node = Node::new(format!("node-{depth}"), "Branch").with_children(vec![node]);
    }
    assert_eq!(
        MindMapData::new(node).validate(),
        Err(MindMapError::TreeTooDeep)
    );
}
