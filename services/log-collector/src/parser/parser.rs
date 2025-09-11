use regex::{Regex, Result};
use serde::{Deserialize, Serialize};

/// Define normalized log output
#[derive(Debug, Serialize, Deserialize)]
struct NormalizedLog {
    timestamp: String,
    level: String,
    message: String,
    metadata: Metadata,
    raw_line: String,
}

/// Define Metadata fields to add using `Metadata Enricher`
#[derive(Debug, Serialize, Deserialize)]
struct Metadata {
    service: String,
    container: String,
    process: String,
    stream: String,
    host: String,
}

/// Define parser supported log formats
enum LogFormat {
    CRI,
    DockerJSON,
    ArbitraryJSON,
    PlainText,
    Syslog(SyslogVariant),
}

/// Define supported Syslogs
enum SyslogVariant {
    RFC3164,
    RFC5424,
}

/// Detects log format from a raw line.
///
/// This function tries to identify whether the log line
/// matches one of the supported log formats in [`LogFormat`],
/// which is defined in [`crate::parser`].
///
/// - If log format does not match `LogFormat::CRI`,
/// `LogFormat::DockerJSON`, `LogFormat::ArbitraryJSON` or
/// `LogFormat::Syslog(SyslogVariant)` detection will default
/// to `LogFormat::PlainText`
///
/// - `LogFormat::Syslog` parsing currently supports `RFC3164` and `RFC5424` format Syslogs. See:
///
/// Returns a [`LogFormat`] enum describing the detected type.
pub async fn detect_format(line: &str) -> LogFormat {
    let cri_re =
        Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z (stdout|stderror) [FP]").unwrap();
    if cri_re.is_match(line) {
        return LogFormat::CRI;
    }

    if let Ok(log) = serde_json::from_str::<serde_json::Value>(line) {
        if log.get("log").is_some() && log.get("time").is_some() {
            return LogFormat::DockerJSON;
        } else {
            return LogFormat::ArbitraryJSON;
        }
    }

    let syslog_rfc: [&str; 2] = [
        r"^<\d+[A-Z][a-z]{2}\s+\d{1,2}\s\d{2}:\d{2}:\d{2}",
        r"^<\d+>\d\s\d{4}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z",
    ];

    let syslog_3164_re = Regex::new(syslog_rfc[0]).unwrap();
    let syslog_5424_re = Regex::new(syslog_rfc[1]).unwrap();

    if syslog_3164_re.is_match(line) {
        return LogFormat::Syslog(SyslogVariant::RFC3164);
    }
    if syslog_5424_re.is_match(line) {
        return LogFormat::Syslog(SyslogVariant::RFC5424);
    }

    LogFormat::PlainText
}

impl NormalizedLog {
    /// Provides parsing for [`LogFormat::CRI`] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::CRI` as input.
    ///
    /// Returns provided `LogFormat::CRI` as a `NormalizedLog` struct.
    pub async fn cri_parser(line: &str) -> NormalizedLog {
        //<timestamp> <output stream> <flag> <message>

        let log = detect_format(line);

        let parts: Vec<&str> = line.splitn(4, ' ').collect();

        let timestamp = DateTime::from_str(parts[0]).unwrap_or_else(|_| Utc::now());
        let stream = parts[1].to_string();
        let flag = parts[2].to_string();
        let message = parts[3].to_string();
        let cri_metadata = Metadata {
            stream: Some(stream),
            flag: Some(flag),
            ..Default::default(),
        };

        NormalizedLog {
            timestamp,
            level: None,
            message,
            metadata: Some(Metadata {
        }
    }

    /// Provides parsing for [`LogFormat::DockerJSON`] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::DockerJSON` as input.
    ///
    /// Returns provided `LogFormat::DockerJSON` as a `NormalizedLog` struct.
    pub async fn docker_parser() -> NormalizedLog {}

    /// Provides parsing for [`LogFormat::ArbitraryJSON`] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::ArbitraryJSON` as input
    ///
    /// Returns provided `LogFormat::ArbitraryJSON` as a `NormalizedLog` struct.
    pub async fn arbitrary_json_parser() -> NormalizedLog {}

    /// Provides parsing for [`LogFormat::PlainText`] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::PlainText`.
    ///
    /// - This is intended for use in scenarios where a log was unidentifiable.
    ///
    /// Returns provided `LogFormat::PlainText` as a `NormalizedLog` struct.
    pub async fn plaintext_parser() -> NormalizedLog {}

    /// Provides parsing for [`LogFormat::Syslog(SyslogVariant)] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::Syslog(SyslogVariant)`.
    ///
    /// - Supported Syslog formats: `RFC3164` and `RFC5424`
    ///
    /// Returns provided `LogFormat::Syslog(SyslogVariant)` as a
    /// `NormalizedLog` struct.
    pub async fn syslog_parser() -> NormalizedLog {}
}
