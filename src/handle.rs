use crate::plugins::{Capability, Command, Plugin, PluginRegistry};
use crate::{
    CoreEvent, MindMapData, MindMapError, MindMapState, Operation, OperationOptions,
    OperationResult,
};
use dioxus::prelude::*;
use std::{collections::BTreeMap, rc::Rc};

type Before = Rc<dyn Fn(&Operation) -> bool>;
type Listener = Rc<dyn Fn(CoreEvent)>;

/// Copyable shared document handle. Mutation always passes through checked core
/// transactions. Individual views own their viewport, editor and mounted nodes.
#[derive(Clone, Copy, PartialEq)]
pub struct MindMapHandle {
    state: Signal<MindMapState>,
    before: Signal<Option<Before>>,
    listener: Signal<Option<Listener>>,
    editor_owner: Signal<Option<usize>>,
    subscribers: Signal<BTreeMap<u64, Listener>>,
    next_subscription: Signal<u64>,
    plugins: Signal<PluginRegistry>,
}

/// Create one shared document for any number of views in the current scope.
/// The initial value is consumed only when the component first mounts.
pub fn use_mind_map(data: MindMapData) -> Result<MindMapHandle, MindMapError> {
    use_hook(|| {
        MindMapState::new(data).map(|state| MindMapHandle {
            state: Signal::new(state),
            before: Signal::new(None),
            listener: Signal::new(None),
            editor_owner: Signal::new(None),
            subscribers: Signal::new(BTreeMap::new()),
            next_subscription: Signal::new(0),
            plugins: Signal::new(PluginRegistry::default()),
        })
    })
}
impl MindMapHandle {
    /// Read a reactive persistent snapshot. No internal mutable signal is exposed.
    pub fn document(self) -> Result<MindMapData, MindMapError> {
        self.state
            .try_read()
            .map(|state| state.data().clone())
            .map_err(|_| MindMapError::StaleOperation)
    }
    /// Current selected IDs, reactively tracked.
    pub fn selection(self) -> Result<Vec<String>, MindMapError> {
        self.state
            .try_read()
            .map(|state| state.selection().to_vec())
            .map_err(|_| MindMapError::StaleOperation)
    }
    /// Current generation, reactively tracked.
    pub fn revision(self) -> Result<u64, MindMapError> {
        self.state
            .try_read()
            .map(|state| state.revision())
            .map_err(|_| MindMapError::StaleOperation)
    }
    /// Whether mutation is disabled.
    pub fn is_read_only(self) -> Result<bool, MindMapError> {
        self.state
            .try_read()
            .map(|state| state.is_read_only())
            .map_err(|_| MindMapError::StaleOperation)
    }
    /// Set a synchronous veto hook. Invoked without a state borrow; reentrant
    /// mutations cause the outer operation to fail with StaleOperation.
    pub fn set_before(
        mut self,
        hook: impl Fn(&Operation) -> bool + 'static,
    ) -> Result<(), MindMapError> {
        *self
            .before
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)? = Some(Rc::new(hook));
        Ok(())
    }
    /// Clear the veto hook.
    pub fn clear_before(mut self) -> Result<(), MindMapError> {
        *self
            .before
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)? = None;
        Ok(())
    }
    /// Install a host event listener. Events are delivered after releasing state.
    pub fn set_listener(
        mut self,
        listener: impl Fn(CoreEvent) + 'static,
    ) -> Result<(), MindMapError> {
        *self
            .listener
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)? = Some(Rc::new(listener));
        Ok(())
    }
    /// Execute a checked mutation using the current before hook.
    pub fn apply(self, operation: Operation) -> Result<OperationResult, MindMapError> {
        self.apply_with(operation, OperationOptions::default())
    }
    /// Execute a checked mutation with explicit recording options.
    pub fn apply_with(
        mut self,
        operation: Operation,
        options: OperationOptions,
    ) -> Result<OperationResult, MindMapError> {
        let revision = self.revision()?;
        if self.is_read_only()? {
            return Err(MindMapError::ReadOnly);
        }
        let before = self
            .before
            .try_read()
            .map_err(|_| MindMapError::StaleOperation)?
            .clone();
        if let Some(before) = before {
            if !before(&operation) {
                return Err(MindMapError::Rejected);
            }
        }
        let result = {
            let mut state = self
                .state
                .try_write()
                .map_err(|_| MindMapError::StaleOperation)?;
            if state.revision() != revision {
                return Err(MindMapError::StaleOperation);
            }
            state.apply_with(operation, options)?
        };
        self.emit(&result.events)?;
        Ok(result)
    }
    /// Select existing IDs, including collapsed descendants.
    pub fn select(mut self, ids: &[String]) -> Result<(), MindMapError> {
        let events = self
            .state
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .select(ids)?;
        self.emit(&events)
    }
    /// Enable/disable all document mutation entry points.
    pub fn set_read_only(mut self, value: bool) -> Result<(), MindMapError> {
        self.state
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .set_read_only(value);
        Ok(())
    }
    /// Undo and restore selection.
    pub fn undo(mut self) -> Result<(), MindMapError> {
        let events = self
            .state
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .undo()?;
        self.emit(&events)
    }
    /// Redo and restore selection.
    pub fn redo(mut self) -> Result<(), MindMapError> {
        let events = self
            .state
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .redo()?;
        self.emit(&events)
    }
    /// Validate and replace the document, clearing selection and history.
    pub fn refresh(mut self, data: MindMapData) -> Result<(), MindMapError> {
        let events = self
            .state
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .refresh(data)?;
        self.emit(&events)
    }
    pub(crate) fn claim_editor(mut self, owner: usize) -> Result<(), MindMapError> {
        *self
            .editor_owner
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)? = Some(owner);
        Ok(())
    }
    pub(crate) fn owns_editor(self, owner: usize) -> Result<bool, MindMapError> {
        Ok(*self
            .editor_owner
            .try_read()
            .map_err(|_| MindMapError::StaleOperation)?
            == Some(owner))
    }
    /// Add an independent listener. Dropping the returned guard unsubscribes it.
    pub fn subscribe(
        mut self,
        listener: impl Fn(CoreEvent) + 'static,
    ) -> Result<Subscription, MindMapError> {
        let id = *self
            .next_subscription
            .try_read()
            .map_err(|_| MindMapError::StaleOperation)?;
        let next = id
            .checked_add(1)
            .ok_or_else(|| MindMapError::InvalidOperation("Subscription ID exhausted".into()))?;
        *self
            .next_subscription
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)? = next;
        self.subscribers
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .insert(id, Rc::new(listener));
        Ok(Subscription { handle: self, id })
    }
    /// Register a plugin for the lifetime of a guard. No document/history mutation.
    pub fn register_plugin(mut self, plugin: Plugin) -> Result<PluginGuard, MindMapError> {
        let id = plugin.id.clone();
        self.plugins
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .register(plugin)?;
        Ok(PluginGuard {
            handle: self,
            id,
            subscriptions: Vec::new(),
        })
    }
    /// Toggle an existing registration. Disabled capabilities disappear reactively.
    pub fn set_plugin_enabled(mut self, id: &str, enabled: bool) -> Result<(), MindMapError> {
        self.plugins
            .try_write()
            .map_err(|_| MindMapError::StaleOperation)?
            .set_enabled(id, enabled)
    }
    /// Test an enabled capability for this document only.
    pub fn has_plugin(self, capability: Capability) -> bool {
        self.plugins
            .try_read()
            .is_ok_and(|plugins| plugins.provides(capability))
    }
    /// Commands supplied by enabled plugins.
    pub fn plugin_commands(self) -> Result<Vec<Command>, MindMapError> {
        Ok(self
            .plugins
            .try_read()
            .map_err(|_| MindMapError::StaleOperation)?
            .commands())
    }
    fn emit(self, events: &[CoreEvent]) -> Result<(), MindMapError> {
        let listener = self
            .listener
            .try_read()
            .map_err(|_| MindMapError::StaleOperation)?
            .clone();
        let subscribers = self
            .subscribers
            .try_read()
            .map_err(|_| MindMapError::StaleOperation)?
            .iter()
            .map(|(id, listener)| (*id, listener.clone()))
            .collect::<Vec<_>>();
        for event in events {
            if let Some(listener) = &listener {
                listener(event.clone());
            }
            for (id, callback) in &subscribers {
                if self
                    .subscribers
                    .try_read()
                    .is_ok_and(|current| current.contains_key(id))
                {
                    callback(event.clone());
                }
            }
        }
        Ok(())
    }
}

/// An event subscription; dropping it releases the callback, including on unmount.
pub struct Subscription {
    handle: MindMapHandle,
    id: u64,
}
impl Drop for Subscription {
    fn drop(&mut self) {
        if let Ok(mut entries) = self.handle.subscribers.try_write() {
            entries.remove(&self.id);
        }
    }
}
/// Registration lifetime and owned event subscriptions for one plugin instance.
pub struct PluginGuard {
    handle: MindMapHandle,
    id: String,
    subscriptions: Vec<Subscription>,
}
impl PluginGuard {
    /// Attach a listener that is removed together with this plugin.
    pub fn subscribe(
        &mut self,
        listener: impl Fn(CoreEvent) + 'static,
    ) -> Result<(), MindMapError> {
        let handle = self.handle;
        let id = self.id.clone();
        self.subscriptions.push(handle.subscribe(move |event| {
            let enabled = handle
                .plugins
                .try_read()
                .is_ok_and(|registry| registry.is_enabled(&id));
            if enabled {
                listener(event);
            }
        })?);
        Ok(())
    }
    /// Toggle this registration without releasing it.
    pub fn set_enabled(&self, enabled: bool) -> Result<(), MindMapError> {
        self.handle.set_plugin_enabled(&self.id, enabled)
    }
}
impl Drop for PluginGuard {
    fn drop(&mut self) {
        if let Ok(mut registry) = self.handle.plugins.try_write() {
            let _ = registry.unregister(&self.id);
        }
    }
}
/// Install plugins for the current component's lifetime. Returns errors explicitly.
pub fn use_plugins(
    handle: MindMapHandle,
    plugins: Vec<Plugin>,
) -> Result<Rc<Vec<PluginGuard>>, MindMapError> {
    use_hook(|| {
        plugins
            .into_iter()
            .map(|plugin| handle.register_plugin(plugin))
            .collect::<Result<Vec<_>, _>>()
            .map(Rc::new)
    })
}
