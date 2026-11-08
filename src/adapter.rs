//! Compatibility composition for callers that use installed per-user configuration.
//! Embedding consumers can depend directly on nddev-browseruse-client instead.
use crate::{
    BrowserUse, CancellationToken, Error, OwnedSession, protocol::Document, runtime, view,
};
use std::path::PathBuf;

#[derive(Clone)]
pub struct BrowserUseClient {
    inner: nddev_browseruse_client::BrowserUseClient,
}
impl BrowserUseClient {
    pub fn new(socket: PathBuf) -> Result<Self, Error> {
        Ok(Self {
            inner: nddev_browseruse_client::BrowserUseClient::new(socket)?,
        })
    }
    pub fn for_current_user() -> Result<Self, Error> {
        Self::new(runtime::control_socket(&runtime::directory()?)?)
    }
}
impl BrowserUse for BrowserUseClient {
    async fn capabilities(
        &self,
        cancellation: CancellationToken,
    ) -> Result<view::Capabilities, Error> {
        self.inner.capabilities(cancellation).await
    }
    async fn create_owned(
        &self,
        project: &str,
        cancellation: CancellationToken,
    ) -> Result<OwnedSession, Error> {
        self.inner.create_owned(project, cancellation).await
    }
    async fn open_owned(
        &self,
        session: &OwnedSession,
        cancellation: CancellationToken,
    ) -> Result<view::Session, Error> {
        self.inner.open_owned(session, cancellation).await
    }
    async fn execute_owned(
        &self,
        session: &OwnedSession,
        code: &str,
        timeout: std::time::Duration,
        expect: Option<Document>,
        cancellation: CancellationToken,
    ) -> Result<view::Outcome, Error> {
        self.inner
            .execute_owned(session, code, timeout, expect, cancellation)
            .await
    }
    async fn close_owned(
        &self,
        session: &OwnedSession,
        close_tabs: bool,
        cancellation: CancellationToken,
    ) -> Result<(), Error> {
        self.inner
            .close_owned(session, close_tabs, cancellation)
            .await
    }
    async fn status(&self, cancellation: CancellationToken) -> Result<view::Status, Error> {
        self.inner.status(cancellation).await
    }
    async fn open(
        &self,
        project: &str,
        cancellation: CancellationToken,
    ) -> Result<view::Session, Error> {
        self.inner.open(project, cancellation).await
    }
    async fn sessions(&self, cancellation: CancellationToken) -> Result<Vec<view::Session>, Error> {
        self.inner.sessions(cancellation).await
    }
    async fn close(
        &self,
        project: &str,
        close_tabs: bool,
        cancellation: CancellationToken,
    ) -> Result<(), Error> {
        self.inner.close(project, close_tabs, cancellation).await
    }
    async fn update(&self, cancellation: CancellationToken) -> Result<(bool, bool), Error> {
        self.inner.update(cancellation).await
    }
    async fn execute(
        &self,
        project: &str,
        code: &str,
        timeout: std::time::Duration,
        expect: Option<Document>,
        cancellation: CancellationToken,
    ) -> Result<view::Outcome, Error> {
        self.inner
            .execute(project, code, timeout, expect, cancellation)
            .await
    }
}
