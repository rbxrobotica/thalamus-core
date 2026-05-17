use thalamus_core::{ContextEntry, ContextPort, ContextGrant};

/// Stub context port: returns empty context. Production implementations
/// would fetch from authorized data sources.
pub struct StaticContextPort {
    entries: Vec<ContextEntry>,
}

impl StaticContextPort {
    pub fn empty() -> Self {
        Self { entries: vec![] }
    }

    pub fn with_entries(entries: Vec<ContextEntry>) -> Self {
        Self { entries }
    }
}

impl ContextPort for StaticContextPort {
    fn fetch(&self, _grants: &[ContextGrant]) -> Vec<ContextEntry> {
        self.entries.clone()
    }
}
