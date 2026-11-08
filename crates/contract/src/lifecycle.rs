use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Per-host and per-browser cleanup limits. New host/worker instances read them once.
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LifecycleSettings {
    /// Inactivity deadline per session. Executions and explicit recording suspend expiry.
    pub idle_timeout_ms: u64,
    /// Maximum interval between the host's cleanup passes.
    pub sweep_interval_ms: u64,
    /// Recovery grace for a durable control tab whose previous host no longer owns it.
    pub orphan_grace_ms: u64,
    /// Durable group records in one browser profile, including groups awaiting cleanup.
    pub groups_limit: usize,
    /// An active video renews its reservation at this interval, outside execution requests.
    pub recording_heartbeat_ms: u64,
    /// A lost recording owner cannot keep its session alive beyond this reservation.
    pub recording_lease_ms: u64,
}

impl LifecycleSettings {
    pub fn validate(&self) -> bool {
        (1000..=1_800_000).contains(&self.idle_timeout_ms)
            && (100..=10_000).contains(&self.sweep_interval_ms)
            && self.sweep_interval_ms <= self.idle_timeout_ms
            && (100..=30_000).contains(&self.orphan_grace_ms)
            && (1..=64).contains(&self.groups_limit)
            && (1000..=30_000).contains(&self.recording_heartbeat_ms)
            && (3000..=120_000).contains(&self.recording_lease_ms)
            && self.recording_lease_ms >= self.recording_heartbeat_ms * 3
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum VideoLeaseRequest {
    Begin,
    Renew { lease: String },
    Stop { lease: String },
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum VideoLeaseFailure {
    InvalidRequest,
    TargetClosed,
    Capacity,
    Expired,
    Unavailable,
}

/// Session-local operational lease; its opaque ID is never a UI or telemetry field.
#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VideoLeaseReply {
    pub kept: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lease: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heartbeat_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<VideoLeaseFailure>,
}

impl VideoLeaseReply {
    pub fn kept() -> Self {
        Self {
            kept: true,
            lease: None,
            heartbeat_ms: None,
            error: None,
        }
    }
    pub fn refused(error: VideoLeaseFailure) -> Self {
        Self {
            kept: false,
            lease: None,
            heartbeat_ms: None,
            error: Some(error),
        }
    }
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupPhase {
    Starting,
    Ready,
    Running,
    Recording,
    Closing,
}

/// Browser-local recovery identity, never a native session capability or a grant to execute code.
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupIdentity {
    pub id: String,
    pub project: String,
    pub created_at_ms: u64,
}

/// Bounded aggregate counters. No operation arguments or page-content fields are retained here.
#[derive(Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupMetrics {
    pub completed: u64,
    pub failed: u64,
    pub total_ms: u64,
    pub last_ms: Option<u64>,
}

/// Owner-only operational projection. It contains no session capability or execution code.
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupStatus {
    pub project: String,
    pub phase: GroupPhase,
    pub uptime_ms: u64,
    pub idle_ms: u64,
    pub idle_timeout_ms: u64,
    pub site_tabs: usize,
    /// Absent while observation is Off. Revocation discards the aggregate history.
    pub metrics: Option<GroupMetrics>,
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupFailure {
    Unavailable,
    RequestFailed,
    OwnershipConflict,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GroupDashboard {
    pub identity: Option<GroupIdentity>,
    pub connected: bool,
    pub observation: super::Observation,
    pub status: Option<GroupStatus>,
    pub error: Option<GroupFailure>,
}
