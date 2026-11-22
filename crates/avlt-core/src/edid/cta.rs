//! CTA-861 extension block.
use serde::Serialize;
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CtaBlock {
    pub problems: Vec<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AudioDescriptor;
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HdmiForumBlock;
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct HdmiVsdb;
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Svd;
pub fn decode_cta(_index: usize, _b: &[u8; 128]) -> CtaBlock {
    CtaBlock { problems: vec![] }
}
