// External crate
use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tracing::instrument;

/// Define normalized log output
#[derive(Default, Debug, Deserialize, Clone)]
pub struct NormalizedLog {
    pub timestamp: DateTime<Utc>,
    pub level: Option<String>,
    pub message: String,
    pub metadata: Option<Metadata>,
    pub raw_line: String,
}

/// Define Metadata fields to add using `Metadata Enricher`
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Metadata {
    pub stream: String,
    pub flag: Option<String>,
}

/// Helper DockerJSONLog struct to deserialized Docker JSON logs
#[derive(Debug, Deserialize)]
struct DockerLog {
    log: String,
    stream: String,
    time: String,
}

#[derive(Debug, Deserialize)]
struct ArbitraryJsonSchema {
    pub time: String,
    pub level: Option<String>,
    pub msg: Option<String>,
    pub message: Option<String>,
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

impl NormalizedLog {
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
    #[instrument(
        name = "core_agent_parser::format_detection",
        target = "parser::parser::NormalizedLog",
        skip_all,
        level = "debug"
    )]
    pub async fn detect_format(line: &str) -> LogFormat {
        let line = line.trim_start();

        let cri_re = Regex::new(
            r"^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}\.[0-9]+Z (stdout|stderr) [FP]",
        )
        .unwrap();
        if cri_re.is_match(line) {
            tracing::debug!(
                log_line = %line,
                "Log format is Kubernetes CRI, matches Kubernete CRI regex"
            );
            return LogFormat::CRI;
        }

        if let Ok(log) = serde_json::from_slice::<serde_json::Value>(line.as_bytes()) {
            if log.get("log").is_some() && log.get("stream").is_some() && log.get("time").is_some()
            {
                tracing::debug!(
                    log_line = %line,
                    "Log format is Docker JSON, matches DockerJSON"
                );
                return LogFormat::DockerJSON;
            }
            if log.get("time").is_some() {
                tracing::debug!(
                    log_line = %line,
                    "Log format is normal JSON output, matches ArbitraryJSON"
                );
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
            tracing::debug!(
                log_line = %line,
                "Log format is syslog 3164, matches SyslogVariant::RFC3164"
            );
            return LogFormat::Syslog(SyslogVariant::RFC3164);
        }

        if syslog_5424_re.is_match(line) {
            tracing::debug!(
                log_line = %line,
                "Log format is syslog 5424, matches SyslogVariant::RFC5424"
            );
            return LogFormat::Syslog(SyslogVariant::RFC5424);
        }

        tracing::error!(
            log_line = %line,
            "Log format is undefined, does not match any supported format. Unable to process"
        );
        LogFormat::Unknown
    }

    #[instrument(
        name = "core_agent_parser::select_parser",
        target = "parser::parser::NormalizedLog",
        skip_all,
        level = "trace"
    )]
    pub async fn select_parser(line: &str) -> Result<NormalizedLog, String> {
        // Detect the log line's format
        let detected_format = Self::detect_format(line).await;

        // Match this format to an appropriate parser for parsing
        tracing::debug!(
            log_line = %line,
            "Attempting raw log parsing to NormalizedLog format"
        );
        match detected_format {
            LogFormat::CRI => Self::cri_parser(line).await,
            LogFormat::DockerJSON => Self::docker_json_parser(line).await,
            LogFormat::ArbitraryJSON => Self::arbitrary_json_parser(line).await,
            LogFormat::Syslog(_) => Self::syslog_parser(line).await,
            LogFormat::Unknown => Err("Unknown log format".to_string()),
        }
    }

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
    #[instrument(
        name = "core_agent_parser::CRI_parsing",
        target = "parser::parser::NormalizedLog",
        skip_all,
        level = "debug"
    )]
    pub async fn cri_parser(line: &str) -> Result<NormalizedLog, String> {
        tracing::info!(
            log_line = %line,
            "Attempting Kubernetes CRI raw log parsing"
        );
        match Self::detect_format(line).await {
            LogFormat::CRI => {
                let parts: Vec<&str> = line.splitn(4, ' ').collect();

                if parts.len() < 4 {
                    tracing::error!(
                        log_line = %line,
                        "Kubernetes CRI raw log parser failed: insufficient parts in raw log"
                    );
                    return Err(format!(
                        "Attempted parsing for raw log {} failed. Log {} is not CRI format!",
                        line, line
                    ));
                }

                let timestamp = DateTime::from_str(parts[0]).unwrap_or_else(|_| {
                    tracing::warn!(
                        log_line = %line,
                        "Failed to parse Kubernetes CRI raw log timestamp using ::now()"
                    );
                    Utc::now()
                });
                let stream = parts[1].to_string();
                let flag = Some(parts[2].to_string());
                let message = parts[3].to_string();

                tracing::info!(
                    log_line = %line,
                    "Successfully parsed Kubernetes CRI log to NormalizedLog"
                );
                Ok(NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: Some(Metadata { stream, flag }),
                    raw_line: line.to_string(),
                })
            }
            other => {
                tracing::error!(
                    log_line = %line,
                    ?other,
                    "Kubernetes CRI log parser called on wrong raw log format"
                );
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
    #[instrument(
        name = "core_agent_parser::DOCKER_parsing",
        target = "parser::parser::NormalizedLog",
        skip_all,
        level = "debug"
    )]
    pub async fn docker_json_parser(line: &str) -> Result<NormalizedLog, String> {
        tracing::info!(
            log_line = %line,
            "Attempting Docker JSON raw log parsing"
        );
        match Self::detect_format(line).await {
            LogFormat::DockerJSON => {
                let parsed: DockerLog = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::error!(
                            log_line = %line,
                            error = %e,
                            "Failed to decode Docker JSON raw log"
                        );
                        return Err(format!("Docker JSON parsing error: {}", e));
                    }
                };

                let timestamp = DateTime::from_str(&parsed.time).unwrap_or_else(|_| {
                    tracing::warn!(
                        log_line = %line,
                        "Failed to parser Docker JSON timestamp in raw log, using ::now()"
                    );
                    Utc::now()
                });
                let stream = parsed.stream;
                let message = parsed.log.trim_end().to_string();

                tracing::info!(
                    log_line = %line,
                    "Successfully parsed Docker JSON raw log to NormalizedLog"
                );
                Ok(NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: Some(Metadata { stream, flag: None }),
                    raw_line: line.to_string(),
                })
            }
            other => {
                tracing::error!(
                    log_line = %line,
                    ?other,
                    "Docker JSON log parser called on wrong raw log format"
                );
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
    #[instrument(
        name = "core_agent_parser::JSON_parsing",
        target = "parser::parser::NormalizedLog",
        skip_all,
        level = "debug"
    )]
    pub async fn arbitrary_json_parser(line: &str) -> Result<NormalizedLog, String> {
        tracing::info!(
            log_line = %line,
            "Attempting arbitrary JSON raw log parsing"
        );
        match Self::detect_format(line).await {
            LogFormat::ArbitraryJSON => {
                let parsed: ArbitraryJsonSchema = match serde_json::from_str(line) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::error!(
                            log_line = %line,
                            error = %e,
                            "Failed to decode arbitrary JSON raw log"
                        );
                        return Err(format!("Arbitrary JSON raw log parsing error: {}", e));
                    }
                };

                let timestamp = DateTime::from_str(&parsed.time).unwrap_or_else(|_| {
                    tracing::warn!(
                        log_line = %line,
                        "Failed to parse arbitrary JSON raw log timestamp, using ::now()"
                    );
                    Utc::now()
                });
                let message = parsed.msg.or(parsed.message).unwrap_or_default();

                tracing::info!(
                    log_line = %line,
                    "Successfully parsed arbitrary JSON raw log to NormalizedLog"
                );
                Ok(NormalizedLog {
                    timestamp,
                    level: parsed.level,
                    message,
                    metadata: None,
                    raw_line: line.to_string(),
                })
            }
            other => {
                tracing::error!(
                    log_line = %line,
                    ?other,
                    "Arbitrary JSON log parser called on wrong raw log format"
                );
                return Err(format!(
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
    #[instrument(
        name = "core_agent_parser::SYSLOG_parser",
        target = "parser::parser::NormalizedLog",
        skip_all,
        level = "debug"
    )]
    pub async fn syslog_parser(line: &str) -> Result<NormalizedLog, String> {
        tracing::info!(
            log_line = %line,
            "Attempting Syslog raw log parsing"
        );
        match Self::detect_format(line).await {
            // TODO: Improve simplistic log parsing to NormalizedLog
            LogFormat::Syslog(SyslogVariant::RFC5424) => {
                tracing::debug!(
                    log_line = %line,
                    "Parsing RFC5424 raw log to NormalizedLog"
                );
                let parts: Vec<&str> = line.split_whitespace().collect();

                let timestamp = DateTime::from_str(parts[2]).unwrap_or_else(|_| {
                    tracing::warn!(
                        log_line = %line,
                        "Failed to parse RFC5424 raw log timestamp, using ::now()"
                    );
                    Utc::now()
                });
                let message = parts[8].to_string();

                tracing::info!(
                    log_line = %line,
                    "Successfully parsed RFC5424 raw log to NormalizedLog"
                );
                Ok(NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: None,
                    raw_line: line.to_string(),
                })
            }
            // TODO: Improve simplistic log parsing to NormalizedLog
            LogFormat::Syslog(SyslogVariant::RFC3164) => {
                tracing::debug!(
                    log_line = %line,
                    "Parsing RFC3164 raw log to NormalizedLog"
                );
                let parts: Vec<&str> = line.splitn(5, ' ').collect();

                let timestamp = DateTime::from_str(parts[1]).unwrap_or_else(|_| {
                    tracing::warn!(
                        log_line = %line,
                        "Failed to parse RFC3164 raw log to NormalizedLog"
                    );
                    Utc::now()
                });
                let message = parts[4].to_string();

                tracing::info!(
                    log_line = %line,
                    "Successfully parsed RFC3164 raw log to NormalizedLog"
                );
                Ok(NormalizedLog {
                    timestamp,
                    level: None,
                    message,
                    metadata: None,
                    raw_line: line.to_string(),
                })
            }
            other => {
                tracing::error!(
                    log_line = %line,
                    ?other,
                    "Syslog log parser called on wron raw log format"
                );
                return Err(format!(
                    "Unexpected log format, not Syslog RFC5424 or RFC3164: {:?}",
                    other
                ));
            }
        }
    }
}
