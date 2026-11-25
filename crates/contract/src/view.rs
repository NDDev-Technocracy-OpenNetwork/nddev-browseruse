use serde::{Deserialize, Serialize};

// URLs, titles and executor output are private operation results. These
// types deliberately do not implement Debug and never become telemetry.

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub enabled: bool,
    /// The owner's observation profile: full, standard or off (docs/observability.md).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observation: Option<String>,
    pub browser: String,
    /// Version of the connected extension build.
    pub extension: String,
    pub sessions: usize,
    /// The host's own build: its package version and the source commit it was built from.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<Build>,
    /// False when the connected extension speaks another contract than this host: nothing works
    /// until one of them is updated.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compatible: Option<bool>,
    /// False when the host's configured executor runtime cannot start. Omitted when healthy so
    /// older clients receive the unchanged Status shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_ready: Option<bool>,
    /// Fixed privacy-safe failure class when `executor_ready` is false; never a path or stderr.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executor_error: Option<String>,
}

pub use crate::Build;

/// Discovery is a separate operation so an older client receives the unchanged Status shape.
#[derive(Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Capabilities {
    pub protocol: u32,
    pub host: Build,
    pub browser_ready: bool,
    pub owned_sessions: bool,
    pub cancellation: bool,
    pub drained_close: bool,
    pub recording_leases: bool,
    pub group_control: bool,
    pub terminal_groups: bool,
    pub idle_timeout_ms: u64,
    pub max_sessions: usize,
    pub max_tabs: usize,
    pub max_execution_ms: u64,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Session {
    pub project: String,
    pub tabs: usize,
    pub executor: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tab {
    pub target: String,
    /// Main-frame loader of the document Chrome reported when this reply was built.
    pub document: Option<String>,
    pub url: String,
    pub title: String,
}

/// A completed execution. `failure` reports the agent code's own outcome;
/// host refusals are a separate reply and never look like success.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Outcome {
    pub failure: Option<ScriptFailure>,
    pub output: String,
    pub truncated: bool,
    pub tabs: Vec<Tab>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptFailure {
    ScriptError,
    TargetClosed,
    BrowserDisconnected,
    /// The script outlived its deadline and was aborted: its later Playwright calls throw, and
    /// the session keeps its executor and state. An action already sent may still complete.
    Aborted,
}

impl ScriptFailure {
    pub fn code(self) -> &'static str {
        match self {
            Self::ScriptError => "script_error",
            Self::TargetClosed => "target_closed",
            Self::BrowserDisconnected => "browser_disconnected",
            Self::Aborted => "aborted",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthy_status_omits_executor_fields_and_unhealthy_status_serializes_them() {
        let healthy = Status {
            enabled: true,
            observation: Some("full".into()),
            browser: "Chrome/159.0.0.0".into(),
            extension: "0.2.11".into(),
            sessions: 0,
            host: Some(Build {
                version: "0.2.11".into(),
                commit: "0000000".into(),
            }),
            compatible: None,
            executor_ready: None,
            executor_error: None,
        };
        let encoded = serde_json::to_value(&healthy).unwrap();
        assert!(encoded.get("executor_ready").is_none());
        assert!(encoded.get("executor_error").is_none());
        let decoded: Status = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.executor_ready, None);
        assert_eq!(decoded.executor_error, None);

        let broken = Status {
            executor_ready: Some(false),
            executor_error: Some("bun_missing".into()),
            ..healthy
        };
        let encoded = serde_json::to_value(&broken).unwrap();
        assert_eq!(encoded["executor_ready"], false);
        assert_eq!(encoded["executor_error"], "bun_missing");
        let decoded: Status = serde_json::from_value(encoded).unwrap();
        assert_eq!(decoded.executor_ready, Some(false));
        assert_eq!(decoded.executor_error.as_deref(), Some("bun_missing"));
    }
}
