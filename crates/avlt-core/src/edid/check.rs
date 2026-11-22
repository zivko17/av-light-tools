//! EDID validation.
use super::Edid;
use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Severity {
    Ok,
    Warn,
    Error,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Finding {
    pub severity: Severity,
    pub message: String,
}
pub fn check(_e: &Edid) -> Vec<Finding> {
    vec![]
}
