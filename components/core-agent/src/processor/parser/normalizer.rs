use super::types::{
    IntermediateEvent,
    LogEvent,
    Severity,
    LOG_EVENT_VERSION,
};

use chrono::{DateTime, Utc};

#[derive(Clone)]
pub struct Normalizer;

impl Normalizer {
    pub fn normalize(event: IntermediateEvent) -> LogEvent {
        LogEvent {
            version: LOG_EVENT_VERSION,
            timestamp: Self::resolve_timestamp(event.timestamp.as_deref()),
            severity: Self::resolve_severity(event.severity),
            message: Self::resolve_message(event.message, event.raw.as_deref()),
            fields: event.fields,
            raw: event.raw,
            origin: event.origin,
        }
    }

    fn resolve_timestamp(raw: Option<&str>) -> DateTime<Utc> {
        raw.and_then(|s| {
            chrono::DateTime::parse_from_rfc3339(s)
                .ok()
                .map(|dt| dt.with_timezone(&Utc))
        })
        .unwrap_or_else(Utc::now)
    }

    fn resolve_severity(severity: Option<Severity>) -> Severity {
        severity.unwrap_or(Severity::Unknown)
    }

    fn resolve_message(message: Option<String>, raw: Option<&str>) -> String {
        message
            .or_else(|| raw.map(|r| r.to_string()))
            .unwrap_or_default()
    }
}