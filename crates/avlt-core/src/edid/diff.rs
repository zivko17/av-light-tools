//! EDID comparison.
use super::Edid;
use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum DiffKind {
    Field,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiffLine {
    pub field: String,
    pub a: String,
    pub b: String,
}
pub fn diff(_a: &Edid, _b: &Edid) -> Vec<DiffLine> {
    vec![]
}
