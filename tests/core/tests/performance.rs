//! Opt-in operation baseline; excludes renderer/layout and is not a throughput promise.
use dioxus_mind_elixir::{MindMapData, MindMapError, MindMapState, Node, Operation};
use std::time::Instant;

#[test]
#[ignore = "manual baseline: cargo test -p mind-elixir-core-tests --test performance -- --ignored --nocapture"]
fn snapshot_baseline() -> Result<(), MindMapError> {
    for count in [100, 1_000, 5_000] {
        for collapsed in [false, true] {
            let children = (0..count / 10)
                .map(|branch| {
                    let mut parent = Node::new(format!("branch-{branch}"), "分支").with_children(
                        (0..9)
                            .map(|leaf| {
                                Node::new(
                                    format!("leaf-{branch}-{leaf}"),
                                    "中文文本 / performance fixture",
                                )
                            })
                            .collect(),
                    );
                    parent.expanded = !collapsed;
                    parent
                })
                .collect();
            let start = Instant::now();
            let mut state = MindMapState::new(MindMapData::new(
                Node::new("root", "Root").with_children(children),
            ))?;
            let initialization = start.elapsed();
            let start = Instant::now();
            for edit in 0..20 {
                state.apply(Operation::SetTopic {
                    id: "leaf-0-0".into(),
                    topic: format!("Edit {edit}"),
                })?;
            }
            let editing = start.elapsed();
            let start = Instant::now();
            for _ in 0..20 {
                state.undo()?;
            }
            println!(
                "nodes={} collapsed={collapsed} init={initialization:?} edits20={editing:?} undo20={:?}",
                count + 1,
                start.elapsed()
            );
        }
    }
    Ok(())
}
