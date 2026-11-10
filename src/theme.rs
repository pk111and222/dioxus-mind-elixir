//! Themes use the same public CSS variables as host stylesheets.
use crate::MindMapError;
use std::collections::BTreeMap;

/// Explicit optional theme. The default component sets no theme variables, so
/// ancestor CSS remains effective. A supplied theme takes precedence over
/// inherited values; the component's `style` can override the theme again.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Theme {
    /// Human-readable theme name.
    pub name: String,
    /// Main-branch colors, inherited by descendants. Empty uses host CSS.
    pub palette: Vec<String>,
    variables: BTreeMap<String, String>,
}
impl Theme {
    /// Create a theme with no forced visual defaults.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            ..Self::default()
        }
    }
    /// Set a public CSS variable. Reject declaration separators so one value
    /// cannot unexpectedly inject other declarations. Host `style` remains a
    /// normal unrestricted CSS declaration string.
    pub fn set_variable(
        &mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), MindMapError> {
        let name = name.into();
        let value = value.into();
        if !name.starts_with("--dme-")
            || name.len() <= 6
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            || value.trim().is_empty()
            || value.contains([';', '{', '}'])
        {
            return Err(MindMapError::InvalidOperation(
                "Expected a --dme-* variable and one CSS value".into(),
            ));
        }
        self.variables.insert(name, value);
        Ok(())
    }
    /// Variable overrides in this explicit theme.
    pub fn variables(&self) -> &BTreeMap<String, String> {
        &self.variables
    }
    pub(crate) fn css(&self) -> String {
        self.variables
            .iter()
            .map(|(name, value)| format!("{name}:{value};"))
            .collect()
    }
}
