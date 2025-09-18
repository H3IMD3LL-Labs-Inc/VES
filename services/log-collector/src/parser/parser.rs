use chrono::{DateTime, Utc};
use regex::{Regex, Result};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

/// Define normalized log output
#[derive(Debug, Deserialize, Clone)]
pub struct NormalizedLog {
    pub timestamp: DateTime<Utc>,
    pub level: Option<String>,
    pub message: String,
    pub metadata: Option<Metadata>,
    pub raw_line: String,
}

/// Define Metadata fields to add using `Metadata Enricher`
#[derive(Debug, Serialize, Deserialize, Clone)]
struct Metadata {
    stream: String,
    flag: Option<String>,
}

/// Helper DockerJSONLog struct to deserialized Docker JSON logs
#[derive(Debug, Deserialize)]
struct DockerLog {
    log: String,
    stream: String,
    time: String,
}

/// Define parser supported log formats
#[derive(Debug)]
enum LogFormat {
    CRI,
    DockerJSON,
    ArbitraryJSON,
    Syslog(SyslogVariant),
    Unknown,
}

/// Define supported Syslogs
#[derive(Debug)]
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
async fn detect_format(line: &str) -> LogFormat {
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

    LogFormat::Unknown
}

impl NormalizedLog {
    /// Provides parsing for [`LogFormat::CRI`] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::CRI` as input.
    ///
    /// - Currently does not support other Metadata types
    /// except `output stream` and `flag`.
    ///
    /// - Supported CRI log format: `2023-10-06T00:17:09.669794202Z stdout F Your log message here`
    ///
    /// Returns provided `LogFormat::CRI` as a `NormalizedLog` struct.
    pub async fn cri_parser(line: &str) -> Result<NormalizedLog, String> {
        match detect_format(line).await {
            LogFormat::CRI => {
                let parts: Vec<&str> = line.splitn(4, ' ').collect();

                if parts.len() < 4 {
                    Err(eprintln!(
                        "Attempted parsing for log {} failed. Log {} is not CRI format!",
                        line, line
                    ));
                }

                let timestamp = DateTime::from_str(parts[0]).unwrap_or_else(|_| Utc::now());
                let stream = parts[1].to_string();
                let flag = Some(parts[2].to_string());
                let message = parts[3].to_string();

                NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: Some(Metadata { stream, flag }),
                    raw_line: line.to_string(),
                }
            }
            other => {
                return Err(format!(
                    "Unexpected log format, not Kubernetes CRI: {:?}",
                    other
                ));
            }
        }
    }

    /// Provides parsing for [`LogFormat::DockerJSON`] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::DockerJSON` as input.
    ///
    /// - Supported log format for Docker Logs: `{"log": "this is a log message\n", "stream": "stdout", "time": "2023-03-22T08:54:39.123456789Z"}`
    ///
    /// Returns provided `LogFormat::DockerJSON` as a `NormalizedLog` struct.
    pub async fn docker_json_parser(line: &str) -> Result<NormalizedLog, String> {
        match detect_format(line).await {
            LogFormat::DockerJSON => {
                let parsed: DockerLog = serde_json::from_str(line)
                    .map_err(|e| format!("Failed to parse Docker JSON log: {}", e))?;

                let timestamp = DateTime::from_str(&parsed.time).unwrap_or_else(|_| Utc::now());
                let stream = parsed.stream;
                let message = parsed.log.trim_end().to_string();

                let normalized = NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: Some(Metadata { stream, flag: None }),
                    raw_line: line.to_string(),
                };
            }
            other => {
                return Err(format!(
                    "Unexpected log format, not Docker-JSON file: {:?}",
                    other
                ));
            }
        }
    }

    /// Provides parsing for [`LogFormat::ArbitraryJSON`] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::ArbitraryJSON` as input
    ///
    /// - [`LogFormat::ArbitraryJSON`] logs are intended to allow
    /// shipping a custom valid JSON object for your log schema.
    ///
    /// - We recommend using a json format similar to the following
    ///  when working with [`crate::parser::NormalizedLog::arbitrary_json_parser()`]:
    ///
    /// ```json
    /// {
    ///     "time": "2025-09-12T16:34:00Z",
    ///     "level": "INFO",
    ///     "msg": "User logged in"
    /// }
    /// ```
    ///
    /// Returns provided `LogFormat::ArbitraryJSON` as a `NormalizedLog` struct.
    pub async fn arbitrary_json_parser(line: &str) -> Result<NormalizedLog, String> {
        match detect_format(line).await {
            LogFormat::ArbitraryJSON => {
                let parts: Vec<&str> = line.splitn(4, ' ').collect();

                let timestamp = DateTime::from_str(parts[0]).unwrap_or_else(|_| Utc::now());
                let level = Some(parts[1].to_string());
                let message = parts[2].to_string();

                NormalizedLog {
                    timestamp,
                    level,
                    message,
                    metadata: None,
                    raw_line: line.to_string(),
                }
            }
            other => {
                return Err(eprintln!(
                    "Unexpected log format, not ArbitraryJSON: {:?}",
                    other
                ));
            }
        }
    }

    /// Provides parsing for [`LogFormat::Syslog(SyslogVariant)] logs.
    /// Requires output from [`crate::parser::detect_format`]
    /// indicating a log is `LogFormat::Syslog(SyslogVariant)`.
    ///
    /// Supported Syslog formats:
    /// - RFC5424: `<PRI>VERSION TIMESTAMP HOSTNAME APP-NAME/PROCESS-NAME PROCESSID MSGID [STRUCTURED-DATA key-value pairs] MESSAGE`
    /// - RFC3164: `<PRI>MMM DD hh:mm:ss HOSTNAME TAG: MESSAGE`
    ///
    /// Returns provided `LogFormat::Syslog(SyslogVariant)` as a
    /// `NormalizedLog` struct.
    pub async fn syslog_parser(line: &str) -> Result<NormalizedLog, String> {
        match detect_format(line).await {
            LogFormat::Syslog(SyslogVariant::RFC5424) => {
                let parts: Vec<&str> = line.split_whitespace().collect();

                let timestamp = DateTime::from_str(parts[2]).unwrap_or_else(|_| Utc::now());
                let message = parts[8].to_string();

                NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: None,
                    raw_line: line.to_string(),
                }
            }
            LogFormat::Syslog(SyslogVariant::RFC3164) => {
                let parts: Vec<&str> = line.splitn(5, ' ').collect();

                let timestamp = DateTime::from_str(parts[1]).unwrap_or_else(|_| Utc::now());
                let message = parts[4].to_string();

                NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: None,
                    raw_line: line.to_string(),
                }
            }
            other => {
                return Err(format!(
                    "Unexpected log format, not Syslog RFC5424 or RFC3164: {:?}",
                    other
                ));
            }
        }
    }
}
