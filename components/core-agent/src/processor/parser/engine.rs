use super::format::{
    Format,
    FormatDetector,
};
use super::traits::Parser;
use super::types::IntermediateEvent;
use super::error::ParserError;
use crate::sources::models::{SourceOrigin, SourcePayload};

use std::collections::HashMap;

pub struct FormatDetectionState {
    pub locked_format: Option<Format>,
    pub votes: HashMap<Format, usize>,
    pub samples: usize,
}

impl FormatDetectionState {
    pub fn new() -> Self {
        Self {
            locked_format: None,
            votes: HashMap::new(),
            samples: 0,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.locked_format.is_some()
    }
}

pub struct FormatDetectionManager {
    states: HashMap<String, FormatDetectionState>,
}

impl FormatDetectionManager {
    pub fn new() -> Self {
        Self {
            states: HashMap::new(),
        }
    }
    pub fn resolve_format(&mut self, payload: &SourcePayload) -> Format {
        let key = source_key(&payload.origin);

        let state = self
            .states
            .entry(key)
            .or_insert_with(FormatDetectionState::new);

        if let Some(format) = state.locked_format {
            return format;
        }

        let result = FormatDetector::detect(payload);

        let vote = state.votes
            .entry(result.format)
            .or_insert(0);
        *vote += 1;

        state.samples += 1;

        if let Some((format, count)) = state.votes.iter().max_by_key(|(_, v)| *v) {
            let ratio = *count as f32 / state.samples as f32;

            if *count > 10 || ratio > 0.8 {
                state.locked_format = Some(*format);
            }
        }

        result.format
    }
}

pub struct ParserRegistry {
    parsers: HashMap<Format, Box<dyn Parser>>,
}

impl ParserRegistry {
    pub fn new() -> Self {
        Self {
            parsers: HashMap::new(),
        }
    }

    pub fn register_parser<P>(
        &mut self,
        format: Format,
        parser: P,
    )
    where
        P: Parser + 'static,
    {
        self.parsers.insert(format, Box::new(parser));
    }

    pub fn get_parser(&self, format: &Format) -> Option<&Box<dyn Parser>> {
        self.parsers.get(format)
    }
}


pub struct ParserConfig {
    format_overrides: HashMap<String, Format>,
}

impl ParserConfig {
    pub fn new() -> Self {
        Self {
            format_overrides: HashMap::new(),
        }
    }

    pub fn set_format_override(
        &mut self,
        origin: &SourceOrigin,
        format: Format,
    ) {
        let key = source_key(origin);
        self.format_overrides.insert(key, format);
    }

    pub fn get_format_override(&self, origin: &SourceOrigin) -> Option<Format> {
        let key = source_key(origin);
        self.format_overrides.get(&key).copied()
    }
}


pub struct ParserEngine {
    registry: ParserRegistry,
    detection: FormatDetectionManager,
    config: Option<ParserConfig>,
}

impl ParserEngine {
    pub fn new(registry: ParserRegistry, config: Option<ParserConfig>) -> Self {
        Self {
            registry,
            detection: FormatDetectionManager::new(),
            config,
        }
    }

    pub fn parse(&mut self, payload: &SourcePayload) -> Result<IntermediateEvent, ParserError> {
        let format = if let Some(config) = &self.config {
            if let Some(f) = config.get_format_override(&payload.origin) {
                f
            } else {
                self.detection.resolve_format(&payload)
            }
        } else {
            self.detection.resolve_format(&payload)
        };

        let parser = self
            .registry
            .get_parser(&format)
            .or_else(|| self.registry.get_parser(&Format::PlainText))
            .ok_or(ParserError::UnsupportedFormat)?;

        match parser.parse_raw_data(payload) {
            Ok(event) => Ok(event),
            Err(_) => {
                let fallback = self
                    .registry
                    .get_parser(&Format::PlainText)
                    .ok_or(ParserError::UnsupportedFormat)?;

                fallback.parse_raw_data(payload)
            }
        }
    }
}

fn source_key(origin: &SourceOrigin) -> String {
    match origin {
        SourceOrigin::File { path, .. } => path.to_string_lossy().to_string(),
        SourceOrigin::Journald { unit, .. } => unit.clone(),
        SourceOrigin::Socket { peer_addr, .. } => peer_addr.to_string(),
    }
}