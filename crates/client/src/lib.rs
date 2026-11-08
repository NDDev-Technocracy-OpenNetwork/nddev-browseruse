//! Independent local client SDK. Callers supply an endpoint and cancellation for every exchange.
//! Importing this crate starts no host, browser, task, subscriber or exporter.
pub use nddev_browseruse_contract::{control as protocol, view};
pub use protocol::OwnedSession;
mod adapter;
mod cancellation;
pub mod io;
pub use adapter::BrowserUseClient;
pub use cancellation::CancellationToken;
pub use io::IoError;
use protocol::{Document, Refusal};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[cfg(any(unix, windows))]
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Refused(Refusal),
    #[error("browser-use response is invalid")]
    InvalidResponse,
    #[error("browser-use is not supported on this platform")]
    UnsupportedPlatform,
}

impl From<Refusal> for Error {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

/// Application port: agents depend on typed sessions and outcomes, not on a socket.
#[cfg(any(unix, windows))]
pub trait BrowserUse {
    /// Discovers this host's contract without creating a session or starting an executor.
    fn capabilities(
        &self,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<view::Capabilities, Error>> + Send;
    /// Reserve before opening, retaining this handle even if opening is canceled.
    /// A lost reserve reply can leave only a bounded, unstarted reservation.
    fn create_owned(
        &self,
        project: &str,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<OwnedSession, Error>> + Send;
    fn open_owned(
        &self,
        session: &OwnedSession,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<view::Session, Error>> + Send;
    fn execute_owned(
        &self,
        session: &OwnedSession,
        code: &str,
        timeout: std::time::Duration,
        expect: Option<Document>,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<view::Outcome, Error>> + Send;
    /// Success acknowledges stop/reap and release for this exact generation.
    /// On a transport failure retain the handle and the unresolved drain state.
    fn close_owned(
        &self,
        session: &OwnedSession,
        close_tabs: bool,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<(), Error>> + Send;
    fn status(
        &self,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<view::Status, Error>> + Send;
    /// Opens or returns the project's session and its existing agent tab group.
    fn open(
        &self,
        project: &str,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<view::Session, Error>> + Send;
    fn sessions(
        &self,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<Vec<view::Session>, Error>> + Send;
    fn close(
        &self,
        project: &str,
        close_tabs: bool,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<(), Error>> + Send;
    /// Reloads an unpacked extension whose installed files are newer, or else ends a host whose
    /// binary an install replaced while no session is open; returns whether each was requested.
    fn update(
        &self,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<(bool, bool), Error>> + Send;
    /// Runs Playwright code in the project's executor. `expect` refuses the run
    /// when that tab's document was replaced or the tab left the session.
    fn execute(
        &self,
        project: &str,
        code: &str,
        timeout: std::time::Duration,
        expect: Option<Document>,
        cancellation: CancellationToken,
    ) -> impl Future<Output = Result<view::Outcome, Error>> + Send;
}
