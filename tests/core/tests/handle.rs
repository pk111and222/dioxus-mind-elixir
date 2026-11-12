//! Exercise Dioxus handle callbacks and disposal without a browser renderer.
use dioxus::prelude::*;
use dioxus_mind_elixir::{CoreEvent, MindMapError, MindMapHandle, Operation, use_mind_map};
use mind_elixir_core_tests::fixture;
use std::{cell::RefCell, rc::Rc};

type TestResult = Result<(), MindMapError>;
#[derive(Clone, Props)]
struct HarnessProps {
    run: Rc<dyn Fn(MindMapHandle) -> TestResult>,
    result: Rc<RefCell<Option<TestResult>>>,
}
impl PartialEq for HarnessProps {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.run, &other.run) && Rc::ptr_eq(&self.result, &other.result)
    }
}
fn harness(props: HarnessProps) -> Element {
    let handle = use_mind_map(fixture());
    use_hook(move || {
        let result = handle.and_then(|handle| (props.run)(handle));
        *props.result.borrow_mut() = Some(result);
    });
    rsx! { div {} }
}
fn run(test: impl Fn(MindMapHandle) -> TestResult + 'static) -> TestResult {
    let result = Rc::new(RefCell::new(None));
    let mut dom = VirtualDom::new_with_props(
        harness,
        HarnessProps {
            run: Rc::new(test),
            result: result.clone(),
        },
    );
    dom.rebuild_in_place();
    result
        .borrow_mut()
        .take()
        .ok_or_else(|| MindMapError::InvalidOperation("Harness did not run".into()))?
}
#[test]
fn veto_has_no_side_effect_and_listener_can_read_committed_state() -> TestResult {
    run(|handle| {
        let before = handle.document()?;
        handle.set_before(|_| false)?;
        assert_eq!(
            handle.apply(Operation::SetCompact(true)),
            Err(MindMapError::Rejected)
        );
        assert_eq!(handle.document()?, before);
        handle.clear_before()?;
        let observed = Rc::new(RefCell::new(Vec::new()));
        let capture = observed.clone();
        handle.set_listener(move |event| {
            capture.borrow_mut().push((event, handle.document()));
        })?;
        handle.apply(Operation::SetCompact(true))?;
        let observed = observed.borrow();
        assert!(
            matches!(observed.first(), Some((CoreEvent::Operation(_), Ok(data))) if data.compact)
        );
        Ok(())
    })
}
#[test]
fn reentrant_before_is_revalidated_without_refcell_panics() -> TestResult {
    run(|handle| {
        let callback_result = Rc::new(RefCell::new(None));
        let capture = callback_result.clone();
        handle.set_before(move |_| {
            *capture.borrow_mut() = Some(handle.select(&["css".into()]));
            true
        })?;
        let before = handle.document()?;
        assert_eq!(
            handle.apply(Operation::SetCompact(true)),
            Err(MindMapError::StaleOperation)
        );
        assert_eq!(*callback_result.borrow(), Some(Ok(())));
        assert_eq!(handle.document()?, before);
        assert_eq!(handle.selection()?, ["css"]);
        Ok(())
    })
}
#[test]
fn listener_may_replace_its_registration_after_commit() -> TestResult {
    run(|handle| {
        let result = Rc::new(RefCell::new(None));
        let capture = result.clone();
        handle.set_listener(move |_| {
            *capture.borrow_mut() = Some(handle.set_listener(|_| {}));
        })?;
        handle.apply(Operation::SetCompact(true))?;
        assert_eq!(*result.borrow(), Some(Ok(())));
        Ok(())
    })
}
#[test]
fn disposed_handle_returns_error() -> TestResult {
    let captured = Rc::new(RefCell::new(None));
    let copy = captured.clone();
    run(move |handle| {
        *copy.borrow_mut() = Some(handle);
        Ok(())
    })?;
    let handle = captured.borrow().ok_or(MindMapError::StaleOperation)?;
    assert_eq!(handle.document(), Err(MindMapError::StaleOperation));
    assert_eq!(
        handle.apply(Operation::SetCompact(true)),
        Err(MindMapError::StaleOperation)
    );
    Ok(())
}
#[test]
fn a_draft_survives_selection_changes_before_commit() -> TestResult {
    // Clicking elsewhere selects (and bumps the revision) before the editor
    // blurs and commits; the draft must still apply.
    run(|handle| {
        let original = handle
            .document()?
            .node_data
            .find("css")
            .map(|n| n.topic.clone())
            .ok_or_else(|| MindMapError::NodeNotFound("css".into()))?;
        handle.begin_edit_for_test("css", &original)?;
        handle.update_draft_for_test("renamed");
        let before = handle.revision()?;
        handle.select(&["build".into()])?;
        assert!(handle.revision()? > before);
        assert_eq!(handle.commit_pending(), Ok(true));
        assert_eq!(
            handle
                .document()?
                .node_data
                .find("css")
                .map(|n| n.topic.clone()),
            Some("renamed".into())
        );
        Ok(())
    })
}
#[test]
fn pending_drafts_commit_as_one_undoable_change_or_cancel() -> TestResult {
    run(|handle| {
        assert_eq!(handle.pending_edit()?, None);
        assert_eq!(handle.commit_pending(), Ok(false));
        let original = handle
            .document()?
            .node_data
            .find("css")
            .map(|n| n.topic.clone());
        let Some(original) = original else {
            return Err(MindMapError::NodeNotFound("css".into()));
        };
        handle.begin_edit_for_test("css", &original)?;
        handle.update_draft_for_test("新的\n两行");
        let pending = handle.pending_edit()?;
        assert!(
            pending
                .as_ref()
                .is_some_and(|p| p.is_changed() && p.draft == "新的\n两行")
        );
        assert_eq!(handle.commit_pending(), Ok(true));
        assert_eq!(handle.pending_edit()?, None);
        handle.undo()?;
        assert_eq!(
            handle
                .document()?
                .node_data
                .find("css")
                .map(|n| n.topic.clone()),
            Some(original.clone())
        );
        handle.begin_edit_for_test("css", &original)?;
        handle.update_draft_for_test("   ");
        assert!(handle.pending_edit()?.is_some_and(|p| !p.is_changed()));
        assert_eq!(handle.commit_pending(), Ok(false));
        handle.begin_edit_for_test("css", &original)?;
        handle.update_draft_for_test("丢掉");
        handle.cancel_pending()?;
        assert_eq!(handle.pending_edit()?, None);
        assert_eq!(
            handle
                .document()?
                .node_data
                .find("css")
                .map(|n| n.topic.clone()),
            Some(original)
        );
        Ok(())
    })
}
