use thalamus_core::{ContextEntry, ContextGrant, ContextPort};

/// Stub context port: returns empty context. Production implementations
/// would fetch from authorized data sources.
pub struct StaticContextPort {
    entries: Vec<ContextEntry>,
}

impl StaticContextPort {
    pub fn empty() -> Self {
        Self { entries: vec![] }
    }
}

impl ContextPort for StaticContextPort {
    fn fetch(&self, _grants: &[ContextGrant]) -> Vec<ContextEntry> {
        self.entries.clone()
    }
}
