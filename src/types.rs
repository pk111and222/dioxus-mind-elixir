use std::{collections::HashSet, error::Error, fmt};

use serde::{Deserialize, Serialize};

/// Document layout; numeric values match Mind Elixir JSON.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum LayoutDirection {
    /// All branches left of the root.
    Left,
    /// All branches right of the root.
    #[default]
    Right,
    /// Main branches distributed between both sides.
    Side,
    /// Root above the branches.
    Down,
}
impl TryFrom<u8> for LayoutDirection {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Left),
            1 => Ok(Self::Right),
            2 => Ok(Self::Side),
            3 => Ok(Self::Down),
            _ => Err(format!("Invalid layout direction: {value}")),
        }
    }
}
impl From<LayoutDirection> for u8 {
    fn from(value: LayoutDirection) -> Self {
        match value {
            LayoutDirection::Left => 0,
            LayoutDirection::Right => 1,
            LayoutDirection::Side => 2,
            LayoutDirection::Down => 3,
        }
    }
}

/// Per-main-branch side, independent of the document layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum NodeDirection {
    /// Left side.
    Left,
    /// Right side.
    Right,
}
impl TryFrom<u8> for NodeDirection {
    type Error = String;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Left),
            1 => Ok(Self::Right),
            _ => Err(format!("Invalid node direction: {value}")),
        }
    }
}
impl From<NodeDirection> for u8 {
    fn from(value: NodeDirection) -> Self {
        match value {
            NodeDirection::Left => 0,
            NodeDirection::Right => 1,
        }
    }
}

/// Supported plain-text styles, mapped to upstream field names.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NodeStyle {
    /// CSS font size.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_size: Option<String>,
    /// CSS font family.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_family: Option<String>,
    /// Text color.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Background.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    /// CSS font weight.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font_weight: Option<String>,
    /// Node width.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<String>,
    /// Border shorthand.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub border: Option<String>,
    /// Text decoration.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text_decoration: Option<String>,
}

fn present_json<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(deserializer).map(Some)
}

fn expanded_default() -> bool {
    true
}

/// A plain-text node in the persistent nested tree.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Node {
    /// Stable ID, unique within this document.
    pub id: String,
    /// Plain-text label, including optional line breaks.
    pub topic: String,
    /// Ordered children.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Node>,
    /// Whether children are visible; missing means expanded.
    #[serde(default = "expanded_default")]
    pub expanded: bool,
    /// Side assigned to a main branch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<NodeDirection>,
    /// Explicit per-node appearance.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub style: Option<NodeStyle>,
    /// Explicit branch stroke, inherited by descendants.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_color: Option<String>,
    /// Preserved plain-text note; not rendered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Opaque JSON metadata, including explicit null, preserved without interpretation.
    #[serde(
        default,
        deserialize_with = "present_json",
        skip_serializing_if = "Option::is_none"
    )]
    pub metadata: Option<serde_json::Value>,
}
impl Node {
    /// Construct a leaf.
    pub fn new(id: impl Into<String>, topic: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            topic: topic.into(),
            children: Vec::new(),
            expanded: true,
            direction: None,
            style: None,
            branch_color: None,
            note: None,
            metadata: None,
        }
    }
    /// Set ordered children.
    pub fn with_children(mut self, children: Vec<Node>) -> Self {
        self.children = children;
        self
    }
    /// Find a node, including collapsed descendants.
    pub fn find(&self, id: &str) -> Option<&Node> {
        let mut pending = vec![self];
        while let Some(node) = pending.pop() {
            if node.id == id {
                return Some(node);
            }
            pending.extend(node.children.iter().rev());
        }
        None
    }
    /// Validate this subtree before cloning or inserting externally supplied nodes.
    pub fn validate(&self) -> Result<(), MindMapError> {
        let mut pending = vec![(self, 0usize)];
        let mut ids = HashSet::new();
        while let Some((node, depth)) = pending.pop() {
            if depth > 128 {
                return Err(MindMapError::TreeTooDeep);
            }
            if node.id.trim().is_empty() {
                return Err(MindMapError::EmptyNodeId);
            }
            if !ids.insert(node.id.as_str()) {
                return Err(MindMapError::DuplicateNodeId(node.id.clone()));
            }
            pending.extend(node.children.iter().map(|child| (child, depth + 1)));
        }
        Ok(())
    }
    pub(crate) fn find_mut(&mut self, id: &str) -> Option<&mut Node> {
        if self.id == id {
            return Some(self);
        }
        self.children
            .iter_mut()
            .find_map(|child| child.find_mut(id))
    }
}

/// Nested persistent document. Runtime selection, viewports and history are excluded.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MindMapData {
    /// Root of the document.
    pub node_data: Node,
    /// Layout with upstream numeric encoding.
    #[serde(default)]
    pub direction: LayoutDirection,
    /// Compact spacing (still overridable by host CSS variables).
    #[serde(default)]
    pub compact: bool,
    /// Opaque document metadata, including explicit null.
    #[serde(
        default,
        deserialize_with = "present_json",
        skip_serializing_if = "Option::is_none"
    )]
    pub meta: Option<serde_json::Value>,
}
impl MindMapData {
    /// Construct a right-facing document.
    pub fn new(root: Node) -> Self {
        Self {
            node_data: root,
            direction: LayoutDirection::Right,
            compact: false,
            meta: None,
        }
    }
    /// Reject empty/duplicate IDs and trees deeper than 128 levels.
    pub fn validate(&self) -> Result<(), MindMapError> {
        self.node_data.validate()
    }

    /// Strict JSON import. Unknown fields (including rich content) are errors.
    pub fn from_json(json: &str) -> Result<Self, MindMapError> {
        let mut deserializer = serde_json::Deserializer::from_str(json);
        let document: Self =
            serde_path_to_error::deserialize(&mut deserializer).map_err(|error| {
                MindMapError::InvalidData {
                    path: error.path().to_string(),
                    message: error.inner().to_string(),
                }
            })?;
        deserializer
            .end()
            .map_err(|error| MindMapError::InvalidData {
                path: String::new(),
                message: error.to_string(),
            })?;
        document.validate()?;
        Ok(document)
    }
    /// Validate and export all supported persistent fields.
    pub fn to_json(&self) -> Result<String, MindMapError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(|error| MindMapError::InvalidData {
            path: String::new(),
            message: error.to_string(),
        })
    }
}

/// A recoverable document, operation or renderer failure.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MindMapError {
    /// A node ID was empty.
    EmptyNodeId,
    /// Duplicate ID in one document.
    DuplicateNodeId(String),
    /// Recursive rendering depth limit exceeded.
    TreeTooDeep,
    /// Unknown/invalid JSON field, including its path.
    InvalidData {
        /// Field path.
        path: String,
        /// Parse diagnostic.
        message: String,
    },
    /// Requested node does not exist.
    NodeNotFound(String),
    /// Operation would remove or relocate the root.
    RootOperation,
    /// Destination is inside the moved subtree.
    AncestorMove,
    /// Mutation is disabled.
    ReadOnly,
    /// Before hook vetoed the operation.
    Rejected,
    /// State changed while a callback was running.
    StaleOperation,
    /// History has no entry in this direction.
    NoHistory,
    /// Supplied input is invalid.
    InvalidOperation(String),
    /// Mounted node measurement failed.
    Measurement(String),
    /// Non-finite or empty geometry.
    InvalidGeometry,
}
impl fmt::Display for MindMapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyNodeId => f.write_str("Node IDs must not be empty"),
            Self::DuplicateNodeId(id) => write!(f, "Duplicate node ID: {id}"),
            Self::TreeTooDeep => f.write_str("At most 128 nested levels are supported"),
            Self::InvalidData { path, message } => {
                write!(f, "Invalid document at {path}: {message}")
            }
            Self::NodeNotFound(id) => write!(f, "Node not found: {id}"),
            Self::RootOperation => f.write_str("The root cannot be removed or relocated"),
            Self::AncestorMove => f.write_str("Cannot move a node into its own subtree"),
            Self::ReadOnly => f.write_str("The document is read-only"),
            Self::Rejected => f.write_str("The operation was rejected by the before hook"),
            Self::StaleOperation => f.write_str("The document changed during the operation"),
            Self::NoHistory => f.write_str("No history entry is available"),
            Self::InvalidOperation(message) => write!(f, "Invalid operation: {message}"),
            Self::Measurement(message) => write!(f, "Cannot measure the mind map: {message}"),
            Self::InvalidGeometry => f.write_str("The renderer returned invalid geometry"),
        }
    }
}
impl Error for MindMapError {}
