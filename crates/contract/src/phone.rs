use serde::Deserialize;

/// Private budgets shared by every session on one physical phone browser connection.
/// A running connection retains its values; changes apply to the next connection.
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PhoneLimits {
    /// Messages per outgoing queue; byte budgets also include the active writer.
    pub queue_messages: usize,
    pub browser_queue_bytes: u32,
    pub host_queue_bytes: u32,
    /// Serialized bytes retained by queued or executing native requests.
    pub incoming_bytes: u32,
    pub incoming_messages: usize,
    /// All browser commands, including background setup commands.
    pub pending_calls: usize,
    /// Background operations, including tab adoption and notifications.
    pub background_work: usize,
    pub call_timeout_ms: u64,
    pub write_timeout_ms: u64,
}

impl PhoneLimits {
    pub fn validate(&self) -> bool {
        (1..=1024).contains(&self.queue_messages)
            && (2_097_152..=33_554_432).contains(&self.browser_queue_bytes)
            && (33_554_436..=134_217_728).contains(&self.host_queue_bytes)
            && (2_097_152..=33_554_432).contains(&self.incoming_bytes)
            && (1..=256).contains(&self.incoming_messages)
            && (1..=4096).contains(&self.pending_calls)
            && (1..=128).contains(&self.background_work)
            && (100..=60_000).contains(&self.call_timeout_ms)
            && (100..=60_000).contains(&self.write_timeout_ms)
    }
}
