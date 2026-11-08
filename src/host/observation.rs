//! The owner's observation profile (docs/observability.md). The extension's popup chooses it; the
//! host keeps it, answers `NDS.observation` on a browser session and tells every such session of
//! a change, so each executor collects exactly what the profile allows.

use super::{Host, State};
pub(super) use crate::diagnostics::Observation;
use serde_json::{Value, json};

impl Host {
    /// Applies the profile the extension reports and tells the executors' browser sessions.
    pub(super) fn observe(&self, profile: Observation) {
        self.config.diagnostics.set(profile);
        let mut state = self.state();
        if state.observation == profile {
            return;
        }
        state.observation = profile;
        if profile == Observation::Off {
            for session in state.sessions.values_mut() {
                session.metrics = Default::default();
            }
        }
        // The closed telemetry schema names no profiles, so each has its own event.
        match profile {
            Observation::Full => {
                tracing::info!(event.name = "browser.observation.full", outcome = "ok")
            }
            Observation::Standard => {
                tracing::info!(event.name = "browser.observation.standard", outcome = "ok")
            }
            Observation::Off => {
                tracing::info!(event.name = "browser.observation.off", outcome = "ok")
            }
        }
        // Off and standard collect no content: what the journals hold back is dropped.
        if profile != Observation::Full {
            for session in state.sessions.values_mut() {
                super::capture::revoke(session);
                if let Some(journal) = &session.journal {
                    journal
                        .lock()
                        .unwrap_or_else(|error| error.into_inner())
                        .discard();
                }
            }
        }
        notify(&mut state, &self.config.diagnostics);
    }

    pub(super) fn observation(&self) -> Value {
        let (profile, epoch) = self.config.diagnostics.snapshot();
        json!({ "profile": profile.as_str(), "epoch": epoch })
    }

    pub(super) fn set_private(&self, sites: Vec<String>) {
        let mut state = self.state();
        if state.private_sites == sites {
            return;
        }
        self.config.diagnostics.invalidate();
        state.private_sites = sites;
        let sites = state.private_sites.clone();
        for session in state.sessions.values_mut() {
            super::capture::revoke(session);
            if let Some(journal) = &session.journal {
                let mut writer = journal.lock().unwrap_or_else(|error| error.into_inner());
                writer.discard();
                writer.set_private(&sites);
            }
        }
        notify(&mut state, &self.config.diagnostics);
    }
}

fn notify(state: &mut State, policy: &crate::diagnostics::Policy) {
    let (profile, epoch) = policy.snapshot();
    let profile = profile.as_str();
    let targets: Vec<(String, String)> = state
        .sessions
        .iter()
        .flat_map(|(project, session)| {
            super::relay::browser_aliases(session)
                .into_iter()
                .map(move |alias| (project.clone(), alias))
        })
        .collect();
    for (project, alias) in targets {
        let event = json!({ "method": "NDS.observation", "params": { "profile": profile, "epoch": epoch }, "sessionId": alias });
        super::deliver(state, &project, event);
    }
}

/// The most private sites the owner can keep.
const SITES_LIMIT: usize = 256;
/// The newest journal records the popup shows of an agent group.
const RECENT: usize = 12;

/// Follows the session's recording through its journal records: started, the owner's actions,
/// stopped.
pub(super) fn follow_recording(recording: &mut Option<(String, u64)>, records: &[Value]) {
    for record in records {
        match (record["type"].as_str(), record["event"].as_str()) {
            (Some("recording"), Some("started")) => {
                *recording = record["recording"]
                    .as_str()
                    .filter(|id| id.len() <= 64)
                    .map(|id| (id.to_owned(), 0));
            }
            (Some("recording"), Some("stopped")) => *recording = None,
            (Some("person-action"), _) => {
                if let Some((_, events)) = recording.as_mut()
                    && record["update"].as_bool() != Some(true)
                {
                    *events += 1;
                }
            }
            _ => {}
        }
    }
}

/// The owner's private sites from the extension: lowercase host names, each with its subdomains.
pub(super) fn sites(value: &Value) -> Vec<String> {
    let mut sites: Vec<String> = value
        .as_array()
        .map(|sites| {
            sites
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_ascii_lowercase)
                .filter(|site| {
                    (1..=253).contains(&site.len())
                        && !site.starts_with('.')
                        && !site.ends_with('.')
                        && site
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || b".-".contains(&byte))
                })
                .take(SITES_LIMIT)
                .collect()
        })
        .unwrap_or_default();
    sites.sort();
    sites.dedup();
    sites
}

impl Host {
    /// The popup's view of an agent group's newest journal records, for the owner of this device.
    pub(super) async fn recent(&self, message: &Value) {
        let project = message["group"].as_str().unwrap_or_default().to_owned();
        let directory = self.config.activity.join(&project);
        let lines = if crate::protocol::valid_project(&project) {
            tokio::task::spawn_blocking(move || {
                let query = crate::activity::Query {
                    last: RECENT,
                    ..Default::default()
                };
                crate::activity::read(&directory, &query).unwrap_or_default()
            })
            .await
            .unwrap_or_default()
        } else {
            Vec::new()
        };
        let answer = json!({ "type": "journaled", "request": message["request"], "lines": lines });
        if let Some(framed) = serde_json::to_vec(&answer)
            .ok()
            .and_then(|body| super::frame(&body))
        {
            let _ = self.outbound.try_send(framed);
        }
    }

    /// `NDS.activity { records, flush }`: the executor's raw records for the session's journal,
    /// kept only in the full profile. Writing happens off the async runtime.
    pub(super) async fn activity(&self, project: &str, params: &Value) -> Value {
        let policy = self.config.diagnostics.clone();
        let Some(epoch) = policy
            .content_epoch()
            .filter(|epoch| params["epoch"].as_str() == Some(epoch.token().as_str()))
        else {
            return json!({ "kept": false, "reason": "policy_changed" });
        };
        let Some(records) = params["records"].as_array() else {
            return json!({ "kept": false, "reason": "invalid_request" });
        };
        let limits = &self.config.executor_settings.activity;
        if records.len() > limits.batch_records
            || !crate::protocol::json_fits(records, limits.batch_bytes)
        {
            return json!({ "kept": false, "reason": "capacity" });
        }
        let records = records.clone();
        let flush = params["flush"].as_bool() == Some(true);
        let journal = {
            let mut state = self.state();
            if state.observation != Observation::Full || !policy.accepts_content(epoch) {
                return json!({ "kept": false });
            }
            let sites = state.private_sites.clone();
            let life = state
                .sessions
                .get(project)
                .filter(|session| !session.closing && !session.life.stop.is_cancelled())
                .map(|session| session.life.clone());
            let Some(life) = life else {
                return json!({ "kept": false });
            };
            if let Some(session) = state.sessions.get_mut(project) {
                follow_recording(&mut session.recording, &records);
            }
            Self::open_journal(&mut state, &self.config.activity, project)
                .map(|journal| (journal, sites, life))
        };
        let Some((journal, sites, life)) = journal else {
            return json!({ "kept": false });
        };
        let mut work = super::PendingWork::new(life.clone());
        let kept = tokio::task::spawn_blocking(move || {
            let mut writer = journal.lock().unwrap_or_else(|error| error.into_inner());
            let kept = if policy.accepts_content(epoch) && !life.stop.is_cancelled() {
                writer.scope(epoch.value());
                writer.set_private(&sites);
                let now = std::time::Instant::now();
                for record in &records {
                    if !policy.accepts_content(epoch) || life.stop.is_cancelled() {
                        break;
                    }
                    writer.ingest(record, now);
                }
                if policy.accepts_content(epoch) && !life.stop.is_cancelled() {
                    if flush {
                        writer.flush();
                    } else {
                        writer.tick(now);
                    }
                    true
                } else {
                    writer.discard();
                    false
                }
            } else {
                false
            };
            work.settle();
            kept
        })
        .await
        .unwrap_or(false);
        json!({ "kept": kept })
    }

    /// `NDS.recording { frame, words }`: the recorder's events or one event's frame for agent
    /// code, read from the journal as `recorder events` and `recorder frame` read it.
    pub(super) async fn recording(&self, project: &str, params: &Value) -> Value {
        let frame = params["frame"].as_bool() == Some(true);
        let words: Vec<String> = params["words"]
            .as_array()
            .map(|words| {
                words
                    .iter()
                    .filter_map(Value::as_str)
                    .take(64)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default();
        let journal = self
            .state()
            .sessions
            .get(project)
            .and_then(|session| session.journal.clone());
        let directory = self.config.activity.join(project);
        let answer = tokio::task::spawn_blocking(move || {
            if let Some(journal) = journal {
                journal
                    .lock()
                    .unwrap_or_else(|error| error.into_inner())
                    .flush();
            }
            let words: Vec<&str> = words.iter().map(String::as_str).collect();
            let Some(query) = crate::activity::Recorder::parse(frame, &words) else {
                return json!({ "error": "invalid_request" });
            };
            let found = if frame {
                crate::activity::recorder_frame(&directory, &query)
                    .map(|path| vec![path.display().to_string()])
            } else {
                crate::activity::recorder_events(&directory, &query)
            };
            match found {
                Ok(lines) => json!({ "lines": lines }),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    json!({ "error": "not_found" })
                }
                Err(_) => json!({ "error": "unavailable" }),
            }
        })
        .await;
        answer.unwrap_or_else(|_| json!({ "error": "unavailable" }))
    }

    /// Writes records the host itself observed, in the full profile only.
    pub(super) fn journal(
        state: &mut State,
        root: &std::path::Path,
        project: &str,
        records: &[Value],
        policy: &crate::diagnostics::Policy,
    ) {
        let Some(epoch) = policy.content_epoch() else {
            return;
        };
        if state.observation != Observation::Full {
            return;
        }
        let sites = state.private_sites.clone();
        if let Some(journal) = Self::open_journal(state, root, project) {
            let mut writer = journal.lock().unwrap_or_else(|error| error.into_inner());
            if !policy.accepts_content(epoch) {
                return;
            }
            writer.scope(epoch.value());
            writer.set_private(&sites);
            for record in records {
                if !policy.accepts_content(epoch) {
                    writer.discard();
                    break;
                }
                writer.ingest(record, std::time::Instant::now());
            }
        }
    }

    fn open_journal(
        state: &mut State,
        root: &std::path::Path,
        project: &str,
    ) -> Option<std::sync::Arc<std::sync::Mutex<crate::activity::Writer>>> {
        let session = state.sessions.get_mut(project)?;
        let journal = session.journal.get_or_insert_with(|| {
            let started = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let life: String = session.life.generation.chars().take(8).collect();
            let name = format!("{started:012}-{life}.jsonl");
            std::sync::Arc::new(std::sync::Mutex::new(crate::activity::Writer::new(
                &root.join(project),
                &name,
            )))
        });
        Some(journal.clone())
    }

    /// Writes the records the journals held for folding once nothing identical followed them.
    pub(super) fn tick_journals(&self) {
        let journals: Vec<_> = self
            .state()
            .sessions
            .values()
            .filter_map(|session| session.journal.clone())
            .collect();
        let now = std::time::Instant::now();
        let epoch = self.config.diagnostics.content_epoch();
        for journal in journals {
            let mut writer = journal.lock().unwrap_or_else(|error| error.into_inner());
            if let Some(epoch) =
                epoch.filter(|epoch| self.config.diagnostics.accepts_content(*epoch))
            {
                writer.scope(epoch.value());
                writer.tick(now);
            } else {
                writer.discard();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_full_and_off_are_named_and_anything_else_is_standard() {
        assert_eq!(Observation::parse(&json!("full")), Observation::Full);
        assert_eq!(Observation::parse(&json!("off")), Observation::Off);
        for other in [json!("standard"), json!("FULL"), json!(null), json!(1)] {
            assert_eq!(Observation::parse(&other), Observation::Standard);
        }
        assert_eq!(Observation::Full.as_str(), "full");
    }
}
