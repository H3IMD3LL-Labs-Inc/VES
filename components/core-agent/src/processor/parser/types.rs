use crate::sources::models::SourceOrigin;

use std::collections::HashMap;
use chrono::{DateTime, Utc};

pub const LOG_EVENT_VERSION: u16 = 1;

pub type EventMap = HashMap<String, Value>;

#[derive(Clone)]
pub enum Severity {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
    Unknown,
}

#[derive(Clone)]
pub enum Value {
    String(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
}

#[derive(Clone)]
pub struct LogEvent {
    pub version: u16,
    pub timestamp: DateTime<Utc>,
    pub severity: Severity,
    pub message: String,
    pub fields: EventMap,
    pub raw: Option<String>,
    pub origin: SourceOrigin,
}

#[derive(Clone)]
pub struct IntermediateEvent {
    pub timestamp: Option<String>,
    pub severity: Option<Severity>,
    pub message: Option<String>,
    pub fields: HashMap<String, Value>,
    pub raw: Option<String>,
    pub origin: SourceOrigin,
}
