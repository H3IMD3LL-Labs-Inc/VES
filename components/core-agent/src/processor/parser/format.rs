use crate::sources::models::{SourcePayload, SourceOrigin};

use std::str::from_utf8;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Json,
    Syslog,
    PlainText,
}

pub struct DetectionResult {
    pub format: Format,
    pub confidence: f32,
}

pub struct FormatDetector;

impl FormatDetector {
    pub fn detect(payload: &SourcePayload) -> DetectionResult {
        let raw = payload.raw_data.as_ref();

        if raw.is_empty() {
            return fallback(0.1);
        }

        let data = match from_utf8(raw) {
            Ok(s) => s.as_bytes(),
            Err(_) => return fallback(0.1),
        };

        let line = first_line(data);
        let trimmed = trim_start(line);

        if let Some(format) = payload_origin(&payload.origin) {
            return DetectionResult {
                format,
                confidence: 0.9,
            };
        }

        if looks_like_syslog(data) {
            return DetectionResult {
                format: Format::Syslog,
                confidence: 0.85,
            };
        }

        if looks_like_json(data) {
            return DetectionResult {
                format: Format::Json,
                confidence: 0.75,
            }
        }

        fallback(0.3)
    }
}

fn fallback(confidence: f32) -> DetectionResult {
    DetectionResult {
        format: Format::PlainText,
        confidence,
    }
}

fn payload_origin(origin: &SourceOrigin) -> Option<Format> {
    match origin {
        SourceOrigin::Journald { .. } => Some(Format::Json),
        SourceOrigin::Socket { .. } => Some(Format::Syslog),
        _ => None,
    }
}

fn looks_like_json(data: &[u8]) -> bool {
    let trimmed = trim_start(data);

    match trimmed.first() {
        Some(b'{') | Some(b'[') => {
            trimmed.iter().any(|&b|
                b == b'"' || b == b'}' || b == b']'
            )
        }
        _ => false,
    }
}

fn looks_like_syslog(data: &[u8]) -> bool {
    let data = trim_start(data);

    if data.len() < 4 {
        return false;
    }

    if data[0] == b'<' {
        let mut i = 1;

        while i < data.len() && data[i].is_ascii_digit() {
            i += 1;
        }

        return i < data.len() && data[i] == b'>';
    }

    false
}

fn trim_start(data: &[u8]) -> &[u8] {
    let mut i = 0;

    while i < data.len() && data[i].is_ascii_whitespace() {
        i += 1;
    }

    &data[i..]
}

fn first_line(data: &[u8]) -> &[u8] {
    match data.iter().position(|&b| b == b'\n') {
        Some(pos) => &data[..pos],
        None => data,
    }
}