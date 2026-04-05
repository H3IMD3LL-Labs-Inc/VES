use super::traits::Parser;
use super::types::{IntermediateEvent, Value, Severity};
use super::error::ParserError;
use crate::sources::models::SourcePayload;

use std::collections::HashMap;
use std::str::from_utf8;

pub struct JsonParser;

impl Parser for JsonParser {
    fn parse_raw_data(
        &self,
        payload: &SourcePayload
    ) -> Result<IntermediateEvent, ParserError> {
        let raw = from_utf8(&payload.raw_data)
            .map_err(|_| ParserError::InvalidFormat)?;

        let json: serde_json::Value = serde_json::from_str(raw)
            .map_err(|_| ParserError::InvalidFormat)?;

        let mut fields = HashMap::new();

        if let Some(obj) = json.as_object() {
            for (k, v) in obj {
                fields.insert(k.clone(), json_to_value(v));
            }
        }

        Ok(IntermediateEvent {
            timestamp: extract_timestamp(&json),
            severity: extract_severity(&json),
            message: extract_message(&json),
            fields,
            raw: Some(raw.to_string()),
            origin: payload.origin.clone(),
        })
    }
}

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else if let Some(f) = n.as_f64() {
                Value::Float(f)
            } else {
                Value::Null
            }
        }
        serde_json::Value::Bool(b) => Value::Bool(*b),
        _ => Value::Null,
    }
}

fn extract_timestamp(json: &serde_json::Value) -> Option<String> {
    let obj = json.as_object()?;

    let candidates = ["timestamp", "time", "@timestamp", "ts"];

    for key in candidates {
        if let Some(v) = obj.get(key) {
            if let Some(s) = v.as_str() {
                return Some(s.to_string());
            }
        }
    }

    None
}

fn extract_severity(json: &serde_json::Value) -> Option<Severity> {
    let obj = json.as_object()?;

    let candidates = ["severity", "error", "warn", "info", "debug", "trace"];

    for key in candidates {
        if let Some(v) = obj.get(key) {
            if let Some(s) = v.as_str() {
                return Some(match s.to_lowercase().as_str() {
                    "trace" => Severity::Trace,
                    "debug" => Severity::Debug,
                    "info" => Severity::Info,
                    "warn" | "warning" => Severity::Warn,
                    "error" => Severity::Error,
                    "fatal" => Severity::Fatal,
                    _ => Severity::Unknown,
                });
            }
        }
    }

    None
}

fn extract_message(json: &serde_json::Value) -> Option<String> {
    let obj = json.as_object()?;

    let candidates = ["message", "msg", "log"];

    for key in candidates {
        if let Some(v) = obj.get(key) {
            if let Some(s) = v.as_str() {
                return Some(s.to_string());
            }
        }
    }

    None
}

pub struct PlainTextParser;

impl Parser for PlainTextParser {
    fn parse_raw_data(
        &self,
        payload: &SourcePayload,
    ) -> Result<IntermediateEvent, ParserError> {
        let raw = from_utf8(&payload.raw_data)
            .map_err(|_| ParserError::InvalidEncoding)?;

        Ok(IntermediateEvent {
            timestamp: None,
            severity: None,
            message: Some(raw.to_string()),
            fields: HashMap::new(),
            raw: Some(raw.to_string()),
            origin: payload.origin.clone(),
        })
    }
}

pub struct SyslogParser;

impl Parser for SyslogParser {
    fn parse_raw_data(
        &self,
        payload: &SourcePayload
    ) -> Result<IntermediateEvent, ParserError> {
        let raw = from_utf8(&payload.raw_data)
            .map_err(|_| ParserError::InvalidEncoding)?;

        // TODO: implement propaaaa! RFC parsing

        Ok(IntermediateEvent {
            timestamp: None,
            severity: None,
            message: Some(raw.to_string()),
            fields: HashMap::new(),
            raw: Some(raw.to_string()),
            origin: payload.origin.clone(),
        })
    }
}
