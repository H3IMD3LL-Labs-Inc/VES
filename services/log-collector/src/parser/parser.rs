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
/// Returns a [`LogFormat`] enum describing the detected type.
///
/// `NOTE`: If log format does not match `LogFormat::CRI`,
/// `LogFormat::DockerJSON`, `LogFormat::ArbitraryJSON` or
/// `LogFormat::Syslog(SyslogVariant)` detection will default
/// to `LogFormat::PlainText`
///
/// `LogFormat::Syslog` parsing currently supports `RFC3164` and `RFC5424` format Syslogs. See:
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

/// Parse a supplied log line using a `NormalizedLog` parser type

/// Parsers for supported log formats to return `NormalizedLog` on a log
impl NormalizedLog {
    // CRI Parser
    pub async fn cri_parser() -> NormalizedLog {
        // TODO
    }

    // DockerJSON Parser
    pub async fn docker_parser() -> NormalizedLog {
        // TODO
    }

    // ArbitraryJSON Parser
    pub async fn arbitrary_json_parser() -> NormalizedLog {
        // TODO
    }

    // PlainText Parser
    pub async fn plaintext_parser() -> NormalizedLog {
        // TODO
    }

    // Syslog Parser
    pub async fn syslog_parser() -> NormalizedLog {}
}
