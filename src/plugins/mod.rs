//! Typed optional extensions. All document changes still use core transactions.
pub mod export;
pub mod gesture;
use crate::{MindMapData, MindMapError, MindMapHandle, Node, Operation, Placement};
use std::collections::BTreeMap;

/// Optional interaction capability, scoped to a document handle.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Capability {
    /// Structural shortcuts and navigation.
    Keyboard,
    /// Toggle selection and drag a selection rectangle.
    MultiSelect,
    /// Move selected branches with pointer gestures.
    DragDrop,
    /// Pan when a drag approaches the viewport edge.
    EdgePan,
    /// Context commands.
    ContextMenu,
    /// Toolbar commands.
    Toolbar,
    /// Shared outline.
    Outline,
    /// File export.
    Export,
}
/// A checked command understood by built-in and host menus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    /// Insert a child of the selected node.
    Child,
    /// Insert a sibling after the selected node.
    SiblingAfter,
    /// Insert a sibling before the selected node.
    SiblingBefore,
    /// Insert a new parent.
    Parent,
    /// Delete selected branches.
    Delete,
    /// Copy selected branches after the selection.
    Duplicate,
    /// Toggle children visibility.
    Fold,
    /// Move a node under its previous sibling.
    Indent,
    /// Move a node after its parent.
    Outdent,
    /// Undo a transaction.
    Undo,
    /// Redo a transaction.
    Redo,
}
/// Registration metadata. No dynamic code loader or global registry is used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Plugin {
    /// Stable unique registration key.
    pub id: String,
    /// Declared optional view capabilities.
    pub capabilities: Vec<Capability>,
    /// Commands offered to menus/toolbars.
    pub commands: Vec<Command>,
}
impl Plugin {
    /// Declare a host plugin.
    pub fn new(id: impl Into<String>, capabilities: Vec<Capability>) -> Self {
        Self {
            id: id.into(),
            capabilities,
            commands: Vec::new(),
        }
    }
    /// Standard interaction and presentation plugins, individually removable.
    pub fn builtins() -> Vec<Self> {
        [
            Capability::Keyboard,
            Capability::MultiSelect,
            Capability::DragDrop,
            Capability::EdgePan,
            Capability::ContextMenu,
            Capability::Toolbar,
            Capability::Outline,
            Capability::Export,
        ]
        .into_iter()
        .map(|capability| {
            let mut plugin = Self::new(format!("builtin.{capability:?}"), vec![capability]);
            if matches!(capability, Capability::Toolbar | Capability::ContextMenu) {
                plugin.commands = vec![
                    Command::Child,
                    Command::SiblingAfter,
                    Command::Parent,
                    Command::Duplicate,
                    Command::Fold,
                    Command::Delete,
                    Command::Undo,
                    Command::Redo,
                ];
            }
            plugin
        })
        .collect()
    }
}
/// Minimal registry, usable independently of Dioxus for validation.
#[derive(Clone, Debug, Default)]
pub struct PluginRegistry(BTreeMap<String, (Plugin, bool)>);
impl PluginRegistry {
    /// Register exactly once; duplicate IDs are errors.
    pub fn register(&mut self, plugin: Plugin) -> Result<(), MindMapError> {
        if plugin.id.trim().is_empty() || self.0.contains_key(&plugin.id) {
            return Err(MindMapError::InvalidOperation(
                "Empty or duplicate plugin ID".into(),
            ));
        }
        self.0.insert(plugin.id.clone(), (plugin, true));
        Ok(())
    }
    /// Enable/disable without changing document history.
    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), MindMapError> {
        self.0
            .get_mut(id)
            .ok_or_else(|| MindMapError::InvalidOperation(format!("Unknown plugin: {id}")))?
            .1 = enabled;
        Ok(())
    }
    /// Remove registration and its capabilities.
    pub fn unregister(&mut self, id: &str) -> Result<(), MindMapError> {
        self.0
            .remove(id)
            .ok_or_else(|| MindMapError::InvalidOperation(format!("Unknown plugin: {id}")))?;
        Ok(())
    }
    /// Whether a registration exists and is enabled.
    pub fn is_enabled(&self, id: &str) -> bool {
        self.0.get(id).is_some_and(|(_, enabled)| *enabled)
    }
    /// Whether an enabled plugin provides this capability.
    pub fn provides(&self, capability: Capability) -> bool {
        self.0
            .values()
            .any(|(plugin, enabled)| *enabled && plugin.capabilities.contains(&capability))
    }
    /// Deduplicated commands in registration order.
    pub fn commands(&self) -> Vec<Command> {
        let mut commands = Vec::new();
        for (plugin, enabled) in self.0.values() {
            if *enabled {
                for command in &plugin.commands {
                    if !commands.contains(command) {
                        commands.push(*command);
                    }
                }
            }
        }
        commands
    }
}
/// Every text the components show, supplied by the host. The default is English;
/// hosts replace any field (or all of them) with their own wording or language.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labels {
    /// Topic of nodes created by commands.
    pub new_topic: String,
    /// Command: insert a child.
    pub add_child: String,
    /// Command: insert a sibling after.
    pub add_sibling: String,
    /// Command: insert a sibling before.
    pub insert_before: String,
    /// Command: insert a parent.
    pub insert_parent: String,
    /// Command: delete.
    pub delete: String,
    /// Command: duplicate.
    pub duplicate: String,
    /// Command: expand or collapse.
    pub fold: String,
    /// Command: indent.
    pub indent: String,
    /// Command: outdent.
    pub outdent: String,
    /// Command: undo.
    pub undo: String,
    /// Command: redo.
    pub redo: String,
    /// Accessible name of the map.
    pub map: String,
    /// Accessible name of the outline.
    pub outline: String,
    /// Accessible name of the outline breadcrumbs.
    pub breadcrumbs: String,
    /// Accessible name of the toolbar.
    pub toolbar: String,
    /// Accessible name of the topic editor.
    pub edit_topic: String,
    /// Collapse button.
    pub collapse: String,
    /// Expand button.
    pub expand: String,
    /// Outline row toggle.
    pub toggle_children: String,
    /// Outline row action that focuses a branch.
    pub focus_branch: String,
    /// Marker of a node with a note.
    pub has_note: String,
    /// Marker of a node with a picture.
    pub has_image: String,
    /// Shown when a picture cannot be loaded.
    pub image_unavailable: String,
    /// Status while dragging branches.
    pub moving: String,
    /// Drop target hint.
    pub drop_here: String,
}
impl Default for Labels {
    fn default() -> Self {
        let s = String::from;
        Self {
            new_topic: s("New node"),
            add_child: s("Add child"),
            add_sibling: s("Add sibling"),
            insert_before: s("Insert before"),
            insert_parent: s("Insert parent"),
            delete: s("Delete"),
            duplicate: s("Duplicate"),
            fold: s("Expand / collapse"),
            indent: s("Indent"),
            outdent: s("Outdent"),
            undo: s("Undo"),
            redo: s("Redo"),
            map: s("Mind map"),
            outline: s("Outline"),
            breadcrumbs: s("Breadcrumbs"),
            toolbar: s("Mind map toolbar"),
            edit_topic: s("Edit node"),
            collapse: s("Collapse"),
            expand: s("Expand"),
            toggle_children: s("Toggle children"),
            focus_branch: s("Focus branch"),
            has_note: s("Has note"),
            has_image: s("Has image"),
            image_unavailable: s("Image unavailable"),
            moving: s("Moving branches · Esc to cancel"),
            drop_here: s("Drop to move"),
        }
    }
}
impl Labels {
    /// Label of a command.
    pub fn command(&self, command: Command) -> &str {
        match command {
            Command::Child => &self.add_child,
            Command::SiblingAfter => &self.add_sibling,
            Command::SiblingBefore => &self.insert_before,
            Command::Parent => &self.insert_parent,
            Command::Delete => &self.delete,
            Command::Duplicate => &self.duplicate,
            Command::Fold => &self.fold,
            Command::Indent => &self.indent,
            Command::Outdent => &self.outdent,
            Command::Undo => &self.undo,
            Command::Redo => &self.redo,
        }
    }
}
/// Resolve an ID's parent using the full tree, including collapsed descendants.
pub fn parent<'a>(root: &'a Node, id: &str) -> Option<&'a Node> {
    if root.children.iter().any(|child| child.id == id) {
        Some(root)
    } else {
        root.children.iter().find_map(|child| parent(child, id))
    }
}
/// Resolve a command to a single core operation. IDs are deterministic and checked.
pub fn command_operation(
    data: &MindMapData,
    ids: &[String],
    command: Command,
    new_topic: &str,
) -> Result<Operation, MindMapError> {
    let id = ids
        .first()
        .ok_or_else(|| MindMapError::InvalidOperation("Select a node first".into()))?;
    let node = data
        .node_data
        .find(id)
        .ok_or_else(|| MindMapError::NodeNotFound(id.clone()))?;
    let fresh = || -> Result<Node, MindMapError> {
        let mut n = 1u64;
        loop {
            let candidate = format!("plugin-{n}");
            if data.node_data.find(&candidate).is_none() {
                return Ok(Node::new(candidate, new_topic));
            }
            n = n
                .checked_add(1)
                .ok_or_else(|| MindMapError::InvalidOperation("Node ID space exhausted".into()))?;
        }
    };
    let operation = match command {
        Command::Child => Operation::Insert {
            node: fresh()?,
            placement: Placement::In(id.clone()),
        },
        Command::SiblingAfter => Operation::Insert {
            node: fresh()?,
            placement: Placement::After(id.clone()),
        },
        Command::SiblingBefore => Operation::Insert {
            node: fresh()?,
            placement: Placement::Before(id.clone()),
        },
        Command::Parent => Operation::InsertParent {
            target: id.clone(),
            parent: fresh()?,
        },
        Command::Delete => Operation::Remove(ids.to_vec()),
        Command::Duplicate => Operation::Copy {
            ids: ids.to_vec(),
            placement: Placement::After(id.clone()),
        },
        Command::Fold => Operation::SetExpanded {
            id: id.clone(),
            expanded: !node.expanded,
        },
        Command::Indent => {
            let p = parent(&data.node_data, id).ok_or(MindMapError::RootOperation)?;
            let index = p
                .children
                .iter()
                .position(|n| n.id == *id)
                .ok_or(MindMapError::StaleOperation)?;
            let previous = index
                .checked_sub(1)
                .and_then(|i| p.children.get(i))
                .ok_or_else(|| MindMapError::InvalidOperation("No previous sibling".into()))?;
            Operation::Move {
                ids: ids.to_vec(),
                placement: Placement::In(previous.id.clone()),
            }
        }
        Command::Outdent => {
            let p = parent(&data.node_data, id).ok_or(MindMapError::RootOperation)?;
            if p.id == data.node_data.id {
                return Err(MindMapError::RootOperation);
            }
            Operation::Move {
                ids: ids.to_vec(),
                placement: Placement::After(p.id.clone()),
            }
        }
        Command::Undo | Command::Redo => {
            return Err(MindMapError::InvalidOperation(
                "History command has no tree operation".into(),
            ));
        }
    };
    Ok(operation)
}
/// Execute a command through the shared checked handle. New nodes use `new_topic`.
pub fn execute(
    handle: MindMapHandle,
    command: Command,
    new_topic: &str,
) -> Result<(), MindMapError> {
    match command {
        Command::Undo => handle.undo(),
        Command::Redo => handle.redo(),
        _ => handle
            .apply(command_operation(
                &handle.document()?,
                &handle.selection()?,
                command,
                new_topic,
            )?)
            .map(|_| ()),
    }
}
