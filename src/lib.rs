//! Rust-native Dioxus mind maps, styled through inheritable CSS variables.
//!
//! Renderer-independent checked tree operations, JSON persistence and snapshot
//! history with four HTML/CSS layouts and measured SVG connectors. Use
//! `use_mind_map` and `EditableMindMap` for a shared editable document.
//! Optional typed plugins add interaction, outlines and Rust export formats.
//!
//! # Platform support
//!
//! The default crate enables no renderer. The host selects Dioxus Web or Desktop.
//! Enable the `web` feature for live CSS/font capture to SVG and raster images on
//! `wasm32-unknown-unknown`. Desktop live capture is currently unsupported and
//! returns a recoverable error; tree operations and XMind encoding are portable.
//! Static HTML exports contain no interactive editor. Data compatibility is a
//! strict plain-text subset; unknown JSON fields are rejected.
//!
//! # Optional plugins
//!
//! Keep the guards returned by [`use_plugins`] or [`MindMapHandle::register_plugin`]
//! alive for as long as their capabilities are needed. Views share one handle's
//! document and history, while each view retains its own viewport and editor.
//! Source examples and validation notes are in the project repository.
//!
//! ```rust,no_run
//! use dioxus::prelude::*;
//! use dioxus_mind_elixir::{MindMap, MindMapData, Node};
//!
//! fn app() -> Element {
//!     let data = use_signal(|| MindMapData::new(
//!         Node::new("root", "Ideas").with_children(vec![Node::new("child", "Rust")])
//!     ));
//!     rsx! { MindMap { data } }
//! }
//! ```

mod generate_branch;
mod handle;
mod interact;
mod outline;
mod platform;
pub mod plugins;
mod state;
pub mod text;
mod theme;
mod types;
mod view;

pub use handle::{MindMapHandle, PluginGuard, Subscription, use_mind_map, use_plugins};
pub use interact::{Point, Viewport};
pub use outline::{Outline, OutlineProps, Toolbar, ToolbarProps};
pub use state::{CoreEvent, MindMapState, Operation, OperationOptions, OperationResult, Placement};
pub use theme::Theme;
pub use types::{LayoutDirection, MindMapData, MindMapError, Node, NodeDirection, NodeStyle};
pub use view::{Alignment, EditableMindMap, EditableMindMapProps, MindMap, MindMapProps};

/// Component stylesheet embedded in the crate; no asset-copy step is needed.
pub const STYLESHEET: &str = include_str!("../assets/mind-map.css");
