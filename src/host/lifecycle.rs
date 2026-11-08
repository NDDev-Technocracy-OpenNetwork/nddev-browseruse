//! Owner projections and termination requests. No task content or session capability crosses UI.
use super::{Host, SessionLife, capture, observation::Observation};
use crate::protocol::Refusal;
use nddev_browseruse_contract::{GroupPhase, GroupStatus};
use serde_json::{Value, json};
use std::{sync::Arc, time::Instant};

impl Host {
    pub(super) fn group_status(
        &self,
        project: &str,
        identity: &str,
    ) -> Result<GroupStatus, Refusal> {
        let state = self.state();
        let session = state
            .sessions
            .get(project)
            .filter(|session| session.owner_group.as_deref() == Some(identity))
            .ok_or(Refusal::SessionNotFound)?;
        let phase = if session.closing {
            GroupPhase::Closing
        } else if !session.opened {
            GroupPhase::Starting
        } else if session.running {
            GroupPhase::Running
        } else if session.recording.is_some() || session.capture.is_some() {
            GroupPhase::Recording
        } else {
            GroupPhase::Ready
        };
        let now = Instant::now();
        Ok(GroupStatus {
            project: project.to_owned(),
            phase,
            uptime_ms: now
                .duration_since(session.created)
                .as_millis()
                .min(u64::MAX as u128) as u64,
            idle_ms: now
                .duration_since(session.used)
                .as_millis()
                .min(u64::MAX as u128) as u64,
            idle_timeout_ms: self.config.lifecycle.idle_timeout_ms,
            site_tabs: state
                .tabs
                .values()
                .filter(|tab| tab.project == project)
                .count(),
            metrics: (state.observation != Observation::Off
                && self.config.diagnostics.profile() != Observation::Off)
                .then(|| session.metrics.clone()),
        })
    }

    pub(super) async fn group_message(&self, message: &Value) {
        let Some(request) = message["request"].as_u64() else {
            return;
        };
        let project = message["group"].as_str().unwrap_or_default();
        let identity = message["identity"].as_str().unwrap_or_default();
        let result = match message["action"].as_str() {
            Some("status") => self.group_status(project, identity).and_then(|status| {
                serde_json::to_value(status).map_err(|_| Refusal::InvalidRequest)
            }),
            Some(action @ ("close" | "keep")) => {
                let life = self
                    .state()
                    .sessions
                    .get(project)
                    .filter(|session| session.owner_group.as_deref() == Some(identity))
                    .map(|session| session.life.clone());
                match life {
                    Some(life) => self
                        .close_generation(project, &life, action == "close", true)
                        .await
                        .map(|()| Value::Null),
                    None => Err(Refusal::SessionNotFound),
                }
            }
            _ => Err(Refusal::InvalidRequest),
        };
        let reply = match result {
            Ok(status) => json!({ "type": "grouped", "request": request, "status": status }),
            Err(error) => json!({ "type": "grouped", "request": request, "error": error.code() }),
        };
        if let Some(frame) = serde_json::to_vec(&reply)
            .ok()
            .and_then(|body| super::frame(&body))
        {
            let _ = self.outbound.try_send(frame);
        }
    }

    /// Closing/moving the control tab ends admission immediately. The existing host sweep owns
    /// the asynchronous drain, so no detached cleanup task can be lost at shutdown.
    pub(super) fn group_closed(&self, project: &str, identity: &str) {
        let mut state = self.state();
        if let Some(session) = state
            .sessions
            .get_mut(project)
            .filter(|session| session.owner_group.as_deref() == Some(identity))
        {
            session.closing = true;
            session.close_tabs = true;
            session.life.stop.cancel();
            session.client = None;
            capture::revoke(session);
        }
    }

    pub(super) fn collect_group_metrics(
        &self,
        project: &str,
        life: &Arc<SessionLife>,
        epoch: &str,
        started: Instant,
        failed: bool,
    ) {
        let mut state = self.state();
        if self.config.diagnostics.profile() == Observation::Off
            || self.config.diagnostics.epoch_token() != epoch
        {
            return;
        }
        if let Some(session) = state
            .sessions
            .get_mut(project)
            .filter(|session| Arc::ptr_eq(&session.life, life))
        {
            let duration = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
            session.metrics.completed = session.metrics.completed.saturating_add(1);
            session.metrics.failed = session.metrics.failed.saturating_add(u64::from(failed));
            session.metrics.total_ms = session.metrics.total_ms.saturating_add(duration);
            session.metrics.last_ms = Some(duration);
        }
    }
}
