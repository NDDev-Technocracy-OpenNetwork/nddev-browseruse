//! Local, content-free diagnostics. Embedders keep their own subscriber; installation is explicit.
use serde_json::{Map, Value};
use std::{
    io::Write,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    time::Duration,
};
use tracing::{
    Event, Level, Subscriber,
    field::{Field, Visit},
};
use tracing_subscriber::{
    Layer,
    fmt::{
        format::Writer,
        time::{FormatTime, SystemTime},
    },
    layer::Context,
    prelude::*,
};

const QUEUE_CAPACITY: usize = 512;
const SHUTDOWN_BUDGET: Duration = Duration::from_secs(1);
const MODE_MASK: u64 = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Observation {
    Off = 0,
    #[default]
    Standard = 1,
    Full = 2,
}

impl Observation {
    pub fn parse(value: &Value) -> Self {
        match value.as_str() {
            Some("full") => Self::Full,
            Some("off") => Self::Off,
            _ => Self::Standard,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Standard => "standard",
            Self::Off => "off",
        }
    }
}

/// A host-local collection policy. Its generation prevents replay across a privacy change.
#[derive(Clone, Debug)]
pub struct Policy(Arc<AtomicU64>);

impl Default for Policy {
    fn default() -> Self {
        // The host has not heard the owner's persisted choice yet.
        Self(Arc::new(AtomicU64::new(Observation::Off as u64)))
    }
}

impl Policy {
    pub fn set(&self, profile: Observation) {
        let _ = self
            .0
            .try_update(Ordering::AcqRel, Ordering::Acquire, |old| {
                (old & MODE_MASK != profile as u64)
                    .then(|| (old.wrapping_add(MODE_MASK + 1) & !MODE_MASK) | profile as u64)
            });
    }

    fn capture(&self, level: &Level) -> Option<u64> {
        let state = self.0.load(Ordering::Acquire);
        match state & MODE_MASK {
            0 => None,
            1 if *level > Level::INFO => None,
            _ => Some(state),
        }
    }
}

struct Record {
    policy: u64,
    body: Map<String, Value>,
}

struct LocalLayer {
    policy: Policy,
    sender: SyncSender<Option<Record>>,
    dropped: Arc<AtomicU64>,
    stopping: Arc<AtomicBool>,
}

#[derive(Default)]
struct Fields(Map<String, Value>);

impl Fields {
    fn text(&mut self, field: &Field, value: &str) {
        if !matches!(field.name(), "event.name" | "outcome" | "error.type") {
            return;
        }
        if value.len() <= 128
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
            && (field.name() != "event.name" || value.starts_with("browser."))
        {
            self.0.insert(field.name().into(), value.into());
        }
    }
}

impl Visit for Fields {
    fn record_str(&mut self, field: &Field, value: &str) {
        self.text(field, value);
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        if matches!(
            field.name(),
            "duration_ms" | "bytes" | "count" | "tabs" | "sessions"
        ) {
            self.0.insert(field.name().into(), value.into());
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        if let Ok(value) = u64::try_from(value) {
            self.record_u64(field, value);
        }
    }

    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if matches!(field.name(), "event.name" | "outcome" | "error.type") {
            self.text(field, &format!("{value:?}"));
        }
    }
}

impl<S: Subscriber> Layer<S> for LocalLayer {
    fn on_event(&self, event: &Event<'_>, _context: Context<'_, S>) {
        if self.stopping.load(Ordering::Acquire) {
            return;
        }
        let metadata = event.metadata();
        if !metadata.target().starts_with("nddev_browseruse") {
            return;
        }
        let Some(policy) = self.policy.capture(metadata.level()) else {
            return;
        };
        let mut fields = Fields::default();
        event.record(&mut fields);
        if !fields.0.contains_key("event.name") {
            return;
        }
        let mut timestamp = String::with_capacity(32);
        let _ = SystemTime.format_time(&mut Writer::new(&mut timestamp));
        fields.0.insert("timestamp".into(), timestamp.into());
        fields
            .0
            .insert("severity".into(), metadata.level().to_string().into());
        fields
            .0
            .insert("service.name".into(), "nddev-browseruse".into());
        fields
            .0
            .insert("service.version".into(), env!("CARGO_PKG_VERSION").into());
        fields
            .0
            .insert("deployment.environment.name".into(), "local".into());
        if let Some(commit) = option_env!("BROWSERUSE_SOURCE_COMMIT")
            .filter(|s| s.len() == 40 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            fields.0.insert("service.commit".into(), commit.into());
        }
        if self
            .sender
            .try_send(Some(Record {
                policy,
                body: fields.0,
            }))
            .is_err()
        {
            self.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

/// Bounded shutdown of the local writer; browser protocol stdout is never used.
pub struct Guard {
    sender: SyncSender<Option<Record>>,
    stopping: Arc<AtomicBool>,
    finished: Receiver<()>,
    dropped: Arc<AtomicU64>,
}

impl Guard {
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Release);
        let _ = self.sender.try_send(None);
        let _ = self.finished.recv_timeout(SHUTDOWN_BUDGET);
    }
}

fn writer(
    receiver: Receiver<Option<Record>>,
    policy: Policy,
    stopping: Arc<AtomicBool>,
    dropped: Arc<AtomicU64>,
    mut output: impl Write,
) {
    loop {
        let item = if stopping.load(Ordering::Acquire) {
            receiver.try_recv().ok().flatten()
        } else {
            receiver.recv().ok().flatten()
        };
        let Some(mut record) = item else { break };
        if record.policy != policy.0.load(Ordering::Acquire) {
            continue;
        }
        record.body.insert(
            "telemetry.dropped".into(),
            dropped.load(Ordering::Relaxed).into(),
        );
        let Ok(mut encoded) = serde_json::to_vec(&record.body) else {
            continue;
        };
        encoded.push(b'\n');
        if output.write_all(&encoded).is_err() {
            dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
    let _ = output.flush();
}

fn subscriber(
    policy: Policy,
    output: impl Write + Send + 'static,
) -> std::io::Result<(impl Subscriber + Send + Sync, Guard)> {
    let (sender, receiver) = mpsc::sync_channel(QUEUE_CAPACITY);
    let (done, finished) = mpsc::sync_channel(1);
    let stopping = Arc::new(AtomicBool::new(false));
    let dropped = Arc::new(AtomicU64::new(0));
    let layer = LocalLayer {
        policy: policy.clone(),
        sender: sender.clone(),
        dropped: dropped.clone(),
        stopping: stopping.clone(),
    };
    let stop = stopping.clone();
    let loss = dropped.clone();
    std::thread::Builder::new()
        .name("browseruse-diagnostics".into())
        .spawn(move || {
            writer(receiver, policy, stop, loss, output);
            let _ = done.send(());
        })?;
    Ok((
        tracing_subscriber::registry().with(layer),
        Guard {
            sender,
            stopping,
            finished,
            dropped,
        },
    ))
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("local diagnostics writer is unavailable")]
    Writer(#[from] std::io::Error),
    #[error("a tracing subscriber is already installed")]
    Subscriber(#[from] tracing::subscriber::SetGlobalDefaultError),
}

/// Only an executable composition root calls this; importing the library initializes nothing.
pub fn install(policy: Policy) -> Result<Guard, InstallError> {
    let (subscriber, guard) = subscriber(policy, std::io::stderr())?;
    tracing::subscriber::set_global_default(subscriber)?;
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct Output(Arc<Mutex<Vec<u8>>>);
    impl Write for Output {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn local_diagnostics_do_not_retain_page_fields_or_collect_after_off() {
        let output = Output(Arc::new(Mutex::new(Vec::new())));
        let policy = Policy::default();
        policy.set(Observation::Standard);
        let (collector, guard) = subscriber(policy.clone(), output.clone()).unwrap();
        tracing::subscriber::with_default(collector, || {
            tracing::info!(
                event.name = "browser.test",
                outcome = "ok",
                url = "https://private.invalid",
                code = "private page text",
                count = 2_u64
            );
        });
        drop(guard);
        let bytes = output.0.lock().unwrap().clone();
        let row: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(row["event.name"], "browser.test");
        assert_eq!(row["count"], 2);
        assert!(row.get("url").is_none() && row.get("code").is_none());

        let output = Output(Arc::new(Mutex::new(Vec::new())));
        let policy = Policy::default();
        policy.set(Observation::Off);
        let (subscriber, guard) = subscriber(policy, output.clone()).unwrap();
        tracing::subscriber::with_default(subscriber, || {
            tracing::error!(event.name = "browser.failed", outcome = "error");
        });
        drop(guard);
        assert!(output.0.lock().unwrap().is_empty());
    }

    #[test]
    fn queued_metadata_cannot_reappear_after_collection_is_disabled_and_reenabled() {
        let policy = Policy::default();
        policy.set(Observation::Standard);
        let old = policy.capture(&Level::INFO).unwrap();
        policy.set(Observation::Off);
        policy.set(Observation::Full);
        let (send, receive) = mpsc::sync_channel(1);
        send.send(Some(Record {
            policy: old,
            body: Map::from_iter([("event.name".into(), json!("browser.test"))]),
        }))
        .unwrap();
        drop(send);
        let output = Output(Arc::new(Mutex::new(Vec::new())));
        writer(
            receive,
            policy,
            Arc::new(AtomicBool::new(false)),
            Arc::new(AtomicU64::new(0)),
            output.clone(),
        );
        assert!(output.0.lock().unwrap().is_empty());
    }

    #[test]
    fn startup_diagnostics_wait_for_the_owners_observation_choice() {
        let output = Output(Arc::new(Mutex::new(Vec::new())));
        let (collector, guard) = subscriber(Policy::default(), output.clone()).unwrap();
        tracing::subscriber::with_default(collector, || {
            tracing::error!(event.name = "browser.startup.failed", outcome = "error");
        });
        drop(guard);
        assert!(output.0.lock().unwrap().is_empty());
    }
}
