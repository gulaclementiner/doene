//! Structured errors mirroring Gör't's coded errors (`{ code, params }`).
//!
//! The English `message` is the fallback translation; i18n (a later
//! milestone) translates by `code` with `params`.

/// A structured domain error: a stable machine-readable code, interpolation
/// params, and the English fallback message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodedError {
    /// Stable code, e.g. `"error.recurrenceTypeInvalid"`.
    pub code: &'static str,
    /// English fallback message (already interpolated).
    pub message: String,
    /// Interpolation params in insertion order.
    pub params: Vec<(&'static str, String)>,
}

impl CodedError {
    /// An error with no interpolation params.
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            params: vec![],
        }
    }

    /// A param value by key, if present.
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| v.as_str())
    }
}

impl std::fmt::Display for CodedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CodedError {}
