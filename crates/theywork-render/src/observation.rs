//! Host-reported collection health; reading history never proves live control.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationSummary {
    pub enabled_sources: usize,
    pub scanning: bool,
    pub errors: Vec<String>,
    pub checked_at: i64,
    pub filtered: bool,
}
impl Default for ObservationSummary {
    fn default() -> Self {
        Self {
            enabled_sources: 2,
            scanning: false,
            errors: Vec::new(),
            checked_at: 0,
            filtered: false,
        }
    }
}
