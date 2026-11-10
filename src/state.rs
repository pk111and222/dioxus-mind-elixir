//! Transactional, renderer-independent document operations and snapshot history.
use std::collections::{BTreeMap, BTreeSet};

use crate::{LayoutDirection, MindMapData, MindMapError, Node, NodeDirection};

/// Where an ordered group is inserted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Placement {
    /// Append as children, expanding the target in the same transaction.
    In(String),
    /// Insert immediately before a sibling.
    Before(String),
    /// Insert immediately after a sibling.
    After(String),
}
impl Placement {
    fn target(&self) -> &str {
        match self {
            Self::In(id) | Self::Before(id) | Self::After(id) => id,
        }
    }
}

/// A mutation performed atomically through the core state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operation {
    /// Insert a new subtree.
    Insert {
        /// Subtree, including caller-provided IDs.
        node: Node,
        /// Destination.
        placement: Placement,
    },
    /// Wrap a non-root node in a newly supplied leaf.
    InsertParent {
        /// Existing child.
        target: String,
        /// New parent (must have no children).
        parent: Node,
    },
    /// Remove nodes; selected descendants of selected ancestors are deduplicated.
    Remove(Vec<String>),
    /// Move an ordered group, retaining all IDs.
    Move {
        /// Source IDs, ordered by document traversal.
        ids: Vec<String>,
        /// Destination.
        placement: Placement,
    },
    /// Copy an ordered group, generating new IDs throughout every subtree.
    Copy {
        /// Source IDs.
        ids: Vec<String>,
        /// Destination.
        placement: Placement,
    },
    /// Replace a plain-text topic. Empty/whitespace-only edits are rejected.
    SetTopic {
        /// Node ID.
        id: String,
        /// New text.
        topic: String,
    },
    /// Change visibility of a node's descendants.
    SetExpanded {
        /// Node ID.
        id: String,
        /// New expansion state.
        expanded: bool,
    },
    /// Change document layout.
    SetLayout(LayoutDirection),
    /// Change compact spacing.
    SetCompact(bool),
}

/// Recording options for a single operation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OperationOptions {
    /// Apply without an operation event or independent undo entry. Existing undo
    /// history is retained; redo is invalidated. Prefer one transaction for
    /// auto-expansion plus insertion (already handled by Insert/Move/Copy).
    pub silent: bool,
}

/// Observable core events. Operation is delivered before selection changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CoreEvent {
    /// A successful, changed, non-silent document transaction.
    Operation(Box<Operation>),
    /// Selection now contains these existing IDs, in document order.
    Selection(Vec<String>),
    /// An undo or redo restored a snapshot.
    History {
        /// True for undo, false for redo.
        undo: bool,
    },
    /// A replacement document reset the history baseline.
    Refresh,
}

/// A transaction result; no-op operations return `changed = false`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OperationResult {
    /// Whether the persistent document changed.
    pub changed: bool,
    /// Newly inserted top-level IDs, or moved IDs.
    pub ids: Vec<String>,
    /// Events to deliver after releasing all state borrows.
    pub events: Vec<CoreEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Snapshot {
    data: MindMapData,
    selection: Vec<String>,
}
#[derive(Clone, Debug)]
struct HistoryEntry {
    before: Snapshot,
    after: Snapshot,
}

/// Validated document with private tree/index/selection and bounded history.
///
/// Every mutation is built on a candidate snapshot, validated, then committed.
/// A failed operation cannot change data, selection, index, history or generation.
#[derive(Clone, Debug)]
pub struct MindMapState {
    snapshot: Snapshot,
    parents: BTreeMap<String, Option<String>>,
    undo: Vec<HistoryEntry>,
    redo: Vec<HistoryEntry>,
    history_limit: usize,
    read_only: bool,
    revision: u64,
    next_id: u64,
}
impl MindMapState {
    /// Validate and establish an initial history baseline.
    pub fn new(data: MindMapData) -> Result<Self, MindMapError> {
        data.validate()?;
        let mut state = Self {
            snapshot: Snapshot {
                data,
                selection: Vec::new(),
            },
            parents: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            history_limit: 100,
            read_only: false,
            revision: 0,
            next_id: 0,
        };
        state.normalize_sides();
        state.reindex();
        Ok(state)
    }
    /// Persistent document, without runtime state.
    pub fn data(&self) -> &MindMapData {
        &self.snapshot.data
    }
    /// Current selected IDs.
    pub fn selection(&self) -> &[String] {
        &self.snapshot.selection
    }
    /// Node lookup includes collapsed descendants.
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.data().node_data.find(id)
    }
    /// Parent ID. Root and unknown IDs return None; use `node` to distinguish.
    pub fn parent_id(&self, id: &str) -> Option<&str> {
        self.parents.get(id).and_then(Option::as_deref)
    }
    /// Monotonic generation for stale operation/measurement checks.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Whether document mutations are disabled.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }
    /// Disable/enable every mutation entry, including refresh and history.
    pub fn set_read_only(&mut self, value: bool) {
        if self.read_only != value {
            self.read_only = value;
            self.bump();
        }
    }
    /// Maximum retained undo transactions; zero disables recording.
    pub fn set_history_limit(&mut self, limit: usize) {
        self.history_limit = limit;
        if self.undo.len() > limit {
            self.undo.drain(..self.undo.len() - limit);
        }
        if self.redo.len() > limit {
            self.redo.drain(..self.redo.len() - limit);
        }
    }
    /// Number of available undo and redo entries.
    pub fn history_len(&self) -> (usize, usize) {
        (self.undo.len(), self.redo.len())
    }
    /// Replace selection atomically, preserving traversal order and removing duplicates.
    pub fn select(&mut self, ids: &[String]) -> Result<Vec<CoreEvent>, MindMapError> {
        for id in ids {
            self.require(id)?;
        }
        let wanted: BTreeSet<_> = ids.iter().collect();
        let ordered = ordered_nodes(&self.data().node_data)
            .into_iter()
            .filter(|node| wanted.contains(&node.id))
            .map(|node| node.id.clone())
            .collect::<Vec<_>>();
        if ordered == self.snapshot.selection {
            return Ok(Vec::new());
        }
        self.snapshot.selection = ordered.clone();
        self.bump();
        Ok(vec![CoreEvent::Selection(ordered)])
    }
    /// Execute one checked transaction.
    pub fn apply(&mut self, operation: Operation) -> Result<OperationResult, MindMapError> {
        self.apply_with(operation, OperationOptions::default())
    }
    /// Execute with explicit history/event recording options.
    pub fn apply_with(
        &mut self,
        operation: Operation,
        options: OperationOptions,
    ) -> Result<OperationResult, MindMapError> {
        self.writable()?;
        match &operation {
            Operation::Insert { node, .. } | Operation::InsertParent { parent: node, .. } => {
                node.validate()?
            }
            _ => {}
        }
        let before = self.snapshot.clone();
        let mut candidate = before.clone();
        let mut next_id = self.next_id;
        let ids = self.mutate(&mut candidate, &operation, &mut next_id)?;
        candidate.data.validate()?;
        normalize_sides(&mut candidate.data);
        if candidate.data == before.data {
            return Ok(OperationResult::default());
        }
        candidate
            .selection
            .retain(|id| candidate.data.node_data.find(id).is_some());
        let selection_changed = before.selection != candidate.selection;
        self.snapshot = candidate;
        self.next_id = next_id;
        self.reindex();
        self.bump();
        self.redo.clear();
        let mut events = Vec::new();
        if !options.silent {
            if self.history_limit > 0 {
                self.undo.push(HistoryEntry {
                    before,
                    after: self.snapshot.clone(),
                });
                if self.undo.len() > self.history_limit {
                    self.undo.remove(0);
                }
            }
            events.push(CoreEvent::Operation(Box::new(operation)));
        }
        if selection_changed {
            events.push(CoreEvent::Selection(self.selection().to_vec()));
        }
        Ok(OperationResult {
            changed: true,
            ids,
            events,
        })
    }
    /// Restore the previous document and selection snapshot.
    pub fn undo(&mut self) -> Result<Vec<CoreEvent>, MindMapError> {
        self.writable()?;
        let entry = self.undo.pop().ok_or(MindMapError::NoHistory)?;
        self.snapshot = entry.before.clone();
        self.redo.push(entry);
        self.reindex();
        self.bump();
        Ok(vec![
            CoreEvent::History { undo: true },
            CoreEvent::Selection(self.selection().to_vec()),
        ])
    }
    /// Restore the next snapshot.
    pub fn redo(&mut self) -> Result<Vec<CoreEvent>, MindMapError> {
        self.writable()?;
        let entry = self.redo.pop().ok_or(MindMapError::NoHistory)?;
        self.snapshot = entry.after.clone();
        self.undo.push(entry);
        self.reindex();
        self.bump();
        Ok(vec![
            CoreEvent::History { undo: false },
            CoreEvent::Selection(self.selection().to_vec()),
        ])
    }
    /// Replace the document and reset selection/history. Validation happens first.
    pub fn refresh(&mut self, mut data: MindMapData) -> Result<Vec<CoreEvent>, MindMapError> {
        self.writable()?;
        data.validate()?;
        normalize_sides(&mut data);
        self.snapshot = Snapshot {
            data,
            selection: Vec::new(),
        };
        self.undo.clear();
        self.redo.clear();
        self.reindex();
        self.bump();
        Ok(vec![CoreEvent::Refresh, CoreEvent::Selection(Vec::new())])
    }
    fn require(&self, id: &str) -> Result<&Node, MindMapError> {
        self.node(id)
            .ok_or_else(|| MindMapError::NodeNotFound(id.into()))
    }
    fn writable(&self) -> Result<(), MindMapError> {
        if self.read_only {
            Err(MindMapError::ReadOnly)
        } else {
            Ok(())
        }
    }
    fn bump(&mut self) {
        self.revision = self.revision.saturating_add(1);
    }
    fn normalize_sides(&mut self) {
        normalize_sides(&mut self.snapshot.data);
    }
    fn reindex(&mut self) {
        self.parents.clear();
        let mut pending = vec![(&self.snapshot.data.node_data, None)];
        while let Some((node, parent)) = pending.pop() {
            self.parents.insert(node.id.clone(), parent);
            pending.extend(
                node.children
                    .iter()
                    .map(|child| (child, Some(node.id.clone()))),
            );
        }
    }
    fn sources(&self, ids: &[String]) -> Result<Vec<String>, MindMapError> {
        let wanted: BTreeSet<_> = ids.iter().collect();
        for id in ids {
            self.require(id)?;
            if id == &self.data().node_data.id {
                return Err(MindMapError::RootOperation);
            }
        }
        let mut result = Vec::new();
        for node in ordered_nodes(&self.data().node_data) {
            if !wanted.contains(&node.id) {
                continue;
            }
            let mut parent = self.parent_id(&node.id);
            let mut covered = false;
            while let Some(id) = parent {
                if wanted.iter().any(|wanted| wanted.as_str() == id) {
                    covered = true;
                    break;
                }
                parent = self.parent_id(id);
            }
            if !covered {
                result.push(node.id.clone());
            }
        }
        Ok(result)
    }
    fn mutate(
        &self,
        candidate: &mut Snapshot,
        operation: &Operation,
        next_id: &mut u64,
    ) -> Result<Vec<String>, MindMapError> {
        let root = &mut candidate.data.node_data;
        match operation {
            Operation::SetTopic { id, topic } => {
                if topic.trim().is_empty() {
                    return Err(MindMapError::InvalidOperation(
                        "Topic cannot be empty".into(),
                    ));
                }
                root.find_mut(id)
                    .ok_or_else(|| MindMapError::NodeNotFound(id.clone()))?
                    .topic = topic.clone();
                Ok(Vec::new())
            }
            Operation::SetExpanded { id, expanded } => {
                root.find_mut(id)
                    .ok_or_else(|| MindMapError::NodeNotFound(id.clone()))?
                    .expanded = *expanded;
                Ok(Vec::new())
            }
            Operation::SetLayout(direction) => {
                candidate.data.direction = *direction;
                Ok(Vec::new())
            }
            Operation::SetCompact(compact) => {
                candidate.data.compact = *compact;
                Ok(Vec::new())
            }
            Operation::Insert { node, placement } => {
                let ids = vec![node.id.clone()];
                insert(root, vec![node.clone()], placement)?;
                candidate.selection = ids.clone();
                Ok(ids)
            }
            Operation::InsertParent { target, parent } => {
                self.require(target)?;
                if !parent.children.is_empty() {
                    return Err(MindMapError::InvalidOperation(
                        "Inserted parent must be a leaf".into(),
                    ));
                }
                let parent_id = self.parent_id(target).ok_or(MindMapError::RootOperation)?;
                let siblings = &mut root
                    .find_mut(parent_id)
                    .ok_or_else(|| MindMapError::NodeNotFound(parent_id.into()))?
                    .children;
                let index = siblings
                    .iter()
                    .position(|node| &node.id == target)
                    .ok_or_else(|| MindMapError::NodeNotFound(target.clone()))?;
                let mut wrapper = parent.clone();
                let original = siblings.remove(index);
                wrapper.direction = original.direction;
                wrapper.children.push(original);
                siblings.insert(index, wrapper);
                candidate.selection = vec![parent.id.clone()];
                Ok(candidate.selection.clone())
            }
            Operation::Remove(ids) => {
                let sources = self.sources(ids)?;
                let fallback = sources
                    .first()
                    .and_then(|id| self.parent_id(id))
                    .map(str::to_owned);
                for id in &sources {
                    take(root, id).ok_or_else(|| MindMapError::NodeNotFound(id.clone()))?;
                }
                if !sources.is_empty() {
                    candidate.selection = fallback.into_iter().collect();
                }
                Ok(sources)
            }
            Operation::Move { ids, placement } | Operation::Copy { ids, placement } => {
                let sources = self.sources(ids)?;
                self.require(placement.target())?;
                let copying = matches!(operation, Operation::Copy { .. });
                if !copying {
                    for id in &sources {
                        if self.require(id)?.find(placement.target()).is_some() {
                            return Err(MindMapError::AncestorMove);
                        }
                    }
                }
                let mut nodes = Vec::new();
                let mut used: BTreeSet<String> = ordered_nodes(root)
                    .iter()
                    .map(|node| node.id.clone())
                    .collect();
                for id in &sources {
                    let mut node = if copying {
                        self.require(id)?.clone()
                    } else {
                        take(root, id).ok_or_else(|| MindMapError::NodeNotFound(id.clone()))?
                    };
                    if copying {
                        regenerate_ids(&mut node, &mut used, next_id)?;
                    }
                    nodes.push(node);
                }
                let inserted = nodes.iter().map(|node| node.id.clone()).collect::<Vec<_>>();
                if !nodes.is_empty() {
                    insert(root, nodes, placement)?;
                    candidate.selection = inserted.clone();
                }
                Ok(inserted)
            }
        }
    }
}

fn ordered_nodes(root: &Node) -> Vec<&Node> {
    let mut result = Vec::new();
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        result.push(node);
        pending.extend(node.children.iter().rev());
    }
    result
}
fn take(root: &mut Node, id: &str) -> Option<Node> {
    if let Some(index) = root.children.iter().position(|node| node.id == id) {
        return Some(root.children.remove(index));
    }
    root.children.iter_mut().find_map(|node| take(node, id))
}
fn find_parent_mut<'a>(root: &'a mut Node, id: &str) -> Option<&'a mut Node> {
    if root.children.iter().any(|node| node.id == id) {
        return Some(root);
    }
    root.children
        .iter_mut()
        .find_map(|node| find_parent_mut(node, id))
}
fn insert(
    root: &mut Node,
    mut nodes: Vec<Node>,
    placement: &Placement,
) -> Result<(), MindMapError> {
    let target = placement.target();
    match placement {
        Placement::In(_) => {
            let parent = root
                .find_mut(target)
                .ok_or_else(|| MindMapError::NodeNotFound(target.into()))?;
            parent.expanded = true;
            parent.children.append(&mut nodes);
        }
        Placement::Before(_) | Placement::After(_) => {
            if root.id == target {
                return Err(MindMapError::RootOperation);
            }
            let parent = find_parent_mut(root, target)
                .ok_or_else(|| MindMapError::NodeNotFound(target.into()))?;
            let index = parent
                .children
                .iter()
                .position(|node| node.id == target)
                .ok_or_else(|| MindMapError::NodeNotFound(target.into()))?;
            let side = parent.children.get(index).and_then(|node| node.direction);
            for node in &mut nodes {
                node.direction = side;
            }
            let offset = usize::from(matches!(placement, Placement::After(_)));
            parent
                .children
                .splice(index + offset..index + offset, nodes);
        }
    }
    Ok(())
}
fn regenerate_ids(
    node: &mut Node,
    used: &mut BTreeSet<String>,
    next: &mut u64,
) -> Result<(), MindMapError> {
    loop {
        *next = next
            .checked_add(1)
            .ok_or_else(|| MindMapError::InvalidOperation("Generated ID space exhausted".into()))?;
        let id = format!("dme-{next}");
        if used.insert(id.clone()) {
            node.id = id;
            break;
        }
    }
    for child in &mut node.children {
        regenerate_ids(child, used, next)?;
    }
    Ok(())
}
fn normalize_sides(data: &mut MindMapData) {
    if data.direction != LayoutDirection::Side {
        return;
    }
    let (mut left, mut right) = (0usize, 0usize);
    for child in &mut data.node_data.children {
        let side = match child.direction {
            Some(side) => side,
            None if left <= right => NodeDirection::Left,
            None => NodeDirection::Right,
        };
        child.direction = Some(side);
        match side {
            NodeDirection::Left => left += 1,
            NodeDirection::Right => right += 1,
        }
    }
}
