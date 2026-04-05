use super::types::{IntermediateEvent, LogEvent};
use super::error::ParserError;

use crate::sources::models::SourcePayload;

pub trait Parser {
    fn parse_raw_data(&self, payload: &SourcePayload) -> Result<IntermediateEvent, ParserError>;
}