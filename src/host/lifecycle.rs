//! Owner projections and termination requests. No task content or session capability crosses UI.
use super::{Host, SessionLife, capture, observation::Observation};
use crate::protocol::Refusal;
use nddev_browseruse_contract::{GroupPhase, GroupStatus};
use serde_json::{Value, json};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

impl Host {
    /// Only an acknowledged begin allocates a reservation. A late renew after stop or expiry
    /// cannot recreate it, and every reservation belongs to this exact connected client/life.
    pub(super) fn video_lease(
        &self,
        project: &str,
        client: u64,
        params: &Value,
    ) -> nddev_browseruse_contract::VideoLeaseReply {
        use nddev_browseruse_contract::{
            VideoLeaseFailure as Error, VideoLeaseReply as Reply, VideoLeaseRequest as Request,
        };
        let Ok(request) = serde_json::from_value::<Request>(params.clone()) else {
            return Reply::refused(Error::InvalidRequest);
        };
        if let Request::Renew { lease } | Request::Stop { lease } = &request
            && (lease.len() != 32 || !lease.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Reply::refused(Error::InvalidRequest);
        }
        let now = Instant::now();
        let mut state = self.state();
        if Self::ready(&state).is_err() {
            return Reply::refused(Error::TargetClosed);
        }
        let Some(session) = state.sessions.get_mut(project).filter(|session| {
            session.opened
                && !session.closing
                && !session.life.stop.is_cancelled()
                && session
                    .client
                    .as_ref()
                    .is_some_and(|owner| owner.id == client)
        }) else {
            return Reply::refused(Error::TargetClosed);
        };
        session.video_leases.retain(|_, deadline| *deadline > now);
        match request {
            Request::Begin => {
                if session.video_leases.len() >= crate::protocol::MAX_TABS {
                    return Reply::refused(Error::Capacity);
                }
                let Ok(lease) = super::token() else {
                    return Reply::refused(Error::Unavailable);
                };
                session.video_leases.insert(
                    lease.clone(),
                    now + Duration::from_millis(self.config.lifecycle.recording_lease_ms),
                );
                let mut reply = Reply::kept();
                reply.lease = Some(lease);
                reply.heartbeat_ms = Some(self.config.lifecycle.recording_heartbeat_ms);
                reply
            }
            Request::Renew { lease } => {
                let Some(deadline) = session.video_leases.get_mut(&lease) else {
                    return Reply::refused(Error::Expired);
                };
                *deadline = now + Duration::from_millis(self.config.lifecycle.recording_lease_ms);
                Reply::kept()
            }
            Request::Stop { lease } => {
                session.video_leases.remove(&lease);
                Reply::kept()
            }
        }
    }

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
        } else if !session.video_leases.is_empty() || session.capture.is_some() {
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
