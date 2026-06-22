// Re-export: logger lives in infra/logger.rs for REFACTOR.md compliance.
// Macros (log_info!, log_error!, log_success!) are #[macro_export] at crate root.
pub use crate::infra::logger::*;
