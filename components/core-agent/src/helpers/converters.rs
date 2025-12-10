//! This module defines conversions between internal structs and their protobuf equivalents.
//!
//! These conversions are total (guaranteed to succeed) — they never fail — because every
//! internal field has a valid representation in the protobuf type.

use chrono::{DateTime, Utc};
use prost_types::Timestamp;

use crate::parser::parser::{Metadata as InternalMetadata, NormalizedLog as InternalNormalizedLog};
use crate::proto::common::{Metadata as ProtoMetadata, NormalizedLog as ProtoNormalizedLog};

/// Convert internal `Metadata` (from parser.rs) -> protobuf `Metadata` (from proto::common).
impl From<InternalMetadata> for ProtoMetadata {
    fn from(metadata: InternalMetadata) -> Self {
        Self {
            stream: metadata.stream,
            flag: metadata.flag,
        }
    }
}

/// Convert internal `NormalizedLog` (from parser.rs) -> protobuf `NormalizedLog` (from proto::common).
impl From<InternalNormalizedLog> for ProtoNormalizedLog {
    fn from(log: InternalNormalizedLog) -> Self {
        // Convert chrono::DateTime<Utc> -> prost_types::Timestamp
        // This conversion is always valid and safe
        let timestamp: Option<Timestamp> = Some(Timestamp {
            seconds: log.timestamp.timestamp(),
            nanos: log.timestamp.timestamp_subsec_nanos() as i32,
        });

        Self {
            timestamp,
            level: log.level,
            message: log.message,
            metadata: log.metadata.map(|m| m.into()), // uses the Metadata converter above
            raw_line: log.raw_line,
        }
    }
}
