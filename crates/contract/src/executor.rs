use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Public to the local thin executor. Fixed for one host/executor lifetime; policy stays live.
#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ExecutorSettings {
    pub activity: ActivitySettings,
    pub recorder: RecorderSettings,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivitySettings {
    /// Records and serialized bytes in one native activity command, per executor.
    pub batch_records: usize,
    pub batch_bytes: usize,
    /// Total queued records/bytes, including a batch awaiting delivery; one sender at a time.
    pub queue_records: usize,
    pub queue_bytes: usize,
    pub send_delay_ms: u64,
    /// Maximum safely sized, uncompressed HTTP body read for a journal record.
    pub body_bytes: usize,
    /// Concurrent metadata/body lookups; a slot stays held until the underlying call settles.
    pub network_tasks: usize,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecorderSettings {
    /// Milliseconds between retained frames, per page.
    pub frame_interval_ms: u64,
    pub frame_bytes: usize,
    /// One recording's total retained bytes and duration in milliseconds.
    pub max_bytes: u64,
    pub max_ms: u64,
    pub queued_frames: usize,
    pub queued_bytes: usize,
    pub queued_events: usize,
    pub secret_cache: usize,
    pub secret_timeout_ms: u64,
    pub jpeg_quality: u32,
    pub max_edge: u32,
    pub frame_targets: usize,
}

impl ExecutorSettings {
    pub fn validate(&self) -> bool {
        let a = &self.activity;
        let r = &self.recorder;
        (1..=256).contains(&a.batch_records)
            && (1024..=921_600).contains(&a.batch_bytes)
            && (a.batch_records..=4096).contains(&a.queue_records)
            && (a.batch_bytes..=16_777_216).contains(&a.queue_bytes)
            && (10..=1000).contains(&a.send_delay_ms)
            && (1..=262_144).contains(&a.body_bytes)
            && (1..=64).contains(&a.network_tasks)
            && (100..=5000).contains(&r.frame_interval_ms)
            && (1024..=2_097_152).contains(&r.frame_bytes)
            && (r.frame_bytes as u64..=524_288_000).contains(&r.max_bytes)
            && (1000..=1_200_000).contains(&r.max_ms)
            && (1..=256).contains(&r.queued_frames)
            && (r.frame_bytes..=33_554_432).contains(&r.queued_bytes)
            && (1..=1024).contains(&r.queued_events)
            && (1..=1024).contains(&r.secret_cache)
            && (100..=5000).contains(&r.secret_timeout_ms)
            && (1..=90).contains(&r.jpeg_quality)
            && (320..=1920).contains(&r.max_edge)
            && (1..=64).contains(&r.frame_targets)
    }
}
