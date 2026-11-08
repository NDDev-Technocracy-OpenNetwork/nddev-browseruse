//! Authority for public browser settings and the extension's owner-facing messages.
//! No page content is sent to this module or included in diagnostic formatting.
use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

mod phone;
pub use phone::PhoneLimits;
mod executor;
pub use executor::{ActivitySettings, ExecutionSettings, ExecutorSettings, RecorderSettings};
mod capture;
pub use capture::{CaptureFailure, CaptureReply, CaptureRequest};
mod native;
pub use native::{AdbSettings, NativeSettings};

#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum Observation {
    Full,
    Standard,
    Off,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Build {
    pub version: String,
    pub commit: String,
}

/// Colors projected from the public DS for Chrome APIs that cannot accept CSS tokens.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BadgeColors {
    pub neutral: String,
    pub warning: String,
    pub error: String,
    pub accent: String,
}

/// Public, non-secret settings shipped with this extension. Changes take effect at worker restart.
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendConfig {
    /// Minimum native host version that enforces observation revocation.
    pub minimum_host: String,
    /// Monotonic revision of the prominent disclosure accepted in this browser profile.
    pub consent_revision: u32,
    /// Desired profile for a new installation; effective collection remains Off until consent.
    pub initial_observation: Observation,
    /// Maximum number of private hostnames in this browser profile.
    pub private_sites_limit: u32,
    /// Milliseconds without agent work before the tiny keep-awake capture ends, per tab.
    pub awake_idle_ms: u32,
    /// Milliseconds for one browser setup command, per command.
    pub setup_timeout_ms: u32,
    /// Milliseconds for a popup recorder or journal response, per request.
    pub owner_reply_ms: u32,
    /// Shared maximum in-flight recorder and journal requests, per browser connection.
    pub owner_requests_limit: u32,
    /// Milliseconds between popup activity refreshes; no refresh before consent.
    pub refresh_ms: u32,
    /// Maximum events in one native-message batch, per extension connection.
    pub batch_events: u32,
    /// Maximum estimated bytes in one native-message batch, per extension connection.
    pub batch_bytes: u32,
    /// Maximum joined message bytes, per extension connection.
    pub parts_bytes: u32,
    pub guide_url: String,
    pub privacy_url: String,
    pub install_url: String,
}

/// Values are composed only here; a duplicated, missing or unknown setting is rejected.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Settings {
    pub public: FrontendConfig,
    pub phone_link: PhoneLimits,
    pub executor: ExecutorSettings,
    pub native: NativeSettings,
}

impl Settings {
    pub fn parse(common: &str, profile: &str) -> Result<Self, String> {
        fn merge(into: &mut toml::Value, from: toml::Value) -> Result<(), String> {
            if let (Some(a), Some(b)) = (into.as_table_mut(), from.as_table()) {
                for (key, value) in b {
                    if let Some(previous) = a.get_mut(key) {
                        merge(previous, value.clone())?;
                    } else {
                        a.insert(key.clone(), value.clone());
                    }
                }
                Ok(())
            } else {
                Err("a setting occurs in both configuration files".into())
            }
        }
        let mut common: toml::Value =
            toml::from_str(common).map_err(|_| "invalid common configuration")?;
        merge(
            &mut common,
            toml::from_str(profile).map_err(|_| "invalid profile configuration")?,
        )?;
        let settings: Self = common
            .try_into()
            .map_err(|_| "missing, unknown or mistyped setting")?;
        let public = &settings.public;
        if !settings.phone_link.validate()
            || !settings.executor.validate()
            || !settings.native.validate()
            || public.minimum_host.split('.').count() != 3
            || !public
                .minimum_host
                .split('.')
                .all(|part| part.parse::<u32>().is_ok())
            || public.consent_revision == 0
            || !(1..=256).contains(&public.private_sites_limit)
            || !(1000..=60_000).contains(&public.awake_idle_ms)
            || !(100..=30_000).contains(&public.setup_timeout_ms)
            || !(100..=30_000).contains(&public.owner_reply_ms)
            || !(1..=64).contains(&public.owner_requests_limit)
            || !(500..=60_000).contains(&public.refresh_ms)
            || !(1..=128).contains(&public.batch_events)
            || !(1024..=4_194_304).contains(&public.batch_bytes)
            || !(1024..=33_554_432).contains(&public.parts_bytes)
            || [&public.guide_url, &public.privacy_url, &public.install_url]
                .iter()
                .any(|url| {
                    !url.starts_with(
                        "https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/",
                    )
                })
        {
            return Err("public configuration is outside its declared bounds".into());
        }
        Ok(settings)
    }
}

/// Popup projection; website, group and journal values are local operation results, never logs.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BrowserStatus {
    pub enabled: bool,
    pub observation: Observation,
    pub connected: bool,
    pub consent_required: bool,
    pub host_update_required: bool,
    pub tabs: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host_contract: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub host: Option<Build>,
    pub version: String,
    pub private_sites: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    pub has_tab: bool,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct OwnerRecording {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub events: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(OpenApi)]
#[openapi(
    info(title = "nddev-browseruse local contracts", version = "2"),
    components(schemas(
        FrontendConfig,
        BrowserStatus,
        OwnerRecording,
        BadgeColors,
        ExecutorSettings,
        CaptureRequest,
        CaptureReply
    ))
)]
pub struct Contract;

pub fn schema() -> utoipa::openapi::OpenApi {
    Contract::openapi()
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONFIG: &str = r#"[public]
minimumHost = "1.2.3"
consentRevision = 1
initialObservation = "off"
privateSitesLimit = 32
awakeIdleMs = 2000
setupTimeoutMs = 1000
ownerReplyMs = 1000
ownerRequestsLimit = 32
refreshMs = 500
batchEvents = 16
batchBytes = 4096
partsBytes = 4096
guideUrl = "https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/guide"
privacyUrl = "https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/privacy"
installUrl = "https://github.com/NDDev-Technocracy-OpenNetwork/nddev-browseruse/install"
[phone_link]
queueMessages = 8
browserQueueBytes = 2097152
hostQueueBytes = 33554436
incomingBytes = 2097152
incomingMessages = 8
pendingCalls = 8
backgroundWork = 8
callTimeoutMs = 1000
writeTimeoutMs = 1000
[executor.execution]
pendingSteps = 16
recentSteps = 8
snapshotTimeoutMs = 1000
[executor.activity]
batchRecords = 4
batchBytes = 4096
queueRecords = 16
queueBytes = 8192
sendDelayMs = 20
bodyBytes = 1024
networkTasks = 2
[executor.recorder]
frameIntervalMs = 100
frameBytes = 4096
maxBytes = 8192
maxMs = 5000
queuedFrames = 2
queuedBytes = 8192
queuedEvents = 4
secretCache = 4
secretTimeoutMs = 100
jpegQuality = 70
maxEdge = 1280
frameTargets = 2
[native]
runtimeDirectory = ""
defaultDevice = ""
defaultPhone = ""
executionTimeoutMs = 10000
[native.adb]
server = "127.0.0.1:5037"
connectTimeoutMs = 5000
exchangeTimeoutMs = 10000
shellBytes = 1048576
"#;

    #[test]
    fn settings_have_no_missing_unknown_or_duplicated_values() {
        assert!(Settings::parse(CONFIG, "").is_ok());
        assert!(Settings::parse(CONFIG, "[public]\nconsentRevision=2").is_err());
        assert!(Settings::parse(CONFIG, "[public]\nsecret=1").is_err());
        assert!(Settings::parse(&CONFIG.replace("refreshMs = 500", ""), "").is_err());
    }

    #[test]
    fn wrong_units_bounds_and_external_public_addresses_are_rejected() {
        for (before, after) in [
            ("pendingSteps = 16", "pendingSteps = 33"),
            ("recentSteps = 8", "recentSteps = 0"),
            ("snapshotTimeoutMs = 1000", "snapshotTimeoutMs = 2000"),
            ("refreshMs = 500", "refreshMs = 0"),
            ("consentRevision = 1", "consentRevision = 0"),
            ("batchEvents = 16", "batchEvents = 50000"),
            ("partsBytes = 4096", "partsBytes = 999999999"),
            ("minimumHost = \"1.2.3\"", "minimumHost = \"latest\""),
            ("https://github.com/", "http://example.test/"),
        ] {
            assert!(Settings::parse(&CONFIG.replace(before, after), "").is_err());
        }
    }
}
