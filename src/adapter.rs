use crate::{
    BrowserUse, CancellationToken, Error, IoError, OwnedSession,
    io::{Phase, transport},
    protocol::{self, Document, Header, Reply, Request},
    runtime, socket, view,
};
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    sync::Semaphore,
};

/// Session control and execution use short bounded exchanges; the host, not
/// this client, holds the long-lived browser link.
const CONTROL_DEADLINE: Duration = Duration::from_secs(20);
const EXECUTION_HEADROOM: Duration = Duration::from_secs(4);

#[derive(Clone)]
pub struct BrowserUseClient {
    permits: Arc<Semaphore>,
    socket: PathBuf,
}

impl BrowserUseClient {
    pub fn new(socket: PathBuf) -> Result<Self, Error> {
        if !socket.is_absolute() {
            return Err(IoError::InvalidConfiguration.into());
        }
        Ok(Self {
            permits: Arc::new(Semaphore::new(protocol::MAX_SESSIONS)),
            socket,
        })
    }

    pub fn for_current_user() -> Result<Self, Error> {
        Self::new(runtime::control_socket(&runtime::directory()?)?)
    }

    async fn exchange(
        &self,
        request: Request,
        deadline: Duration,
        cancellation: CancellationToken,
    ) -> Result<Reply, Error> {
        let _permit = self.permits.try_acquire().map_err(|_| IoError::Busy)?;
        let mut frame = serde_json::to_vec(&request).map_err(|_| IoError::InvalidConfiguration)?;
        frame.push(b'\n');
        if frame.len() > protocol::FRAME_LIMIT {
            return Err(IoError::OutputLimit.into());
        }
        let start = Instant::now();
        let payload = tokio::select! {
            biased;
            () = cancellation.cancelled() => Err(IoError::Cancelled),
            result = transfer(&self.socket, &frame, deadline) => result,
        };
        let outcome = match &payload {
            Ok(_) => "ok",
            Err(_) => "error",
        };
        tracing::debug!(
            event.name = "browser.client.exchange",
            duration_ms = start.elapsed().as_millis() as u64,
            outcome,
        );
        match serde_json::from_slice(&payload?).map_err(|_| Error::InvalidResponse)? {
            Reply::Refused { refusal } => Err(refusal.into()),
            reply => Ok(reply),
        }
    }
}

async fn transfer(
    socket: &std::path::Path,
    request: &[u8],
    budget: Duration,
) -> Result<Vec<u8>, IoError> {
    let deadline = tokio::time::Instant::now() + budget;
    let mut stream = stage(deadline, Phase::Connect, socket::connect(socket)).await?;
    stage(deadline, Phase::Request, stream.write_all(request)).await?;
    stage(deadline, Phase::Request, stream.flush()).await?;
    // Keep the same reader for the header and payload: it may already have buffered the
    // payload when the peer closes its write direction after replying.
    let mut reader = BufReader::new(stream);
    let mut header = Vec::new();
    let mut bounded = (&mut reader).take(protocol::FRAME_LIMIT as u64 + 1);
    stage(
        deadline,
        Phase::Header,
        bounded.read_until(b'\n', &mut header),
    )
    .await?;
    if header.len() > protocol::FRAME_LIMIT || header.last() != Some(&b'\n') {
        return Err(IoError::InvalidFrame);
    }
    let header: Header = serde_json::from_slice(&header).map_err(|_| IoError::InvalidResponse)?;
    if header.bytes > protocol::PAYLOAD_LIMIT {
        return Err(IoError::OutputLimit);
    }
    let mut payload = vec![0; header.bytes];
    stage(deadline, Phase::Payload, reader.read_exact(&mut payload)).await?;
    Ok(payload)
}

async fn stage<T>(
    deadline: tokio::time::Instant,
    phase: Phase,
    operation: impl Future<Output = std::io::Result<T>>,
) -> Result<T, IoError> {
    tokio::time::timeout_at(deadline, operation)
        .await
        .map_err(|_| IoError::Timeout(phase))?
        .map_err(|error| transport(error, phase))
}

impl BrowserUse for BrowserUseClient {
    async fn create_owned(
        &self,
        project: &str,
        cancellation: CancellationToken,
    ) -> Result<OwnedSession, Error> {
        match self
            .exchange(
                Request::CreateOwned {
                    project: project.into(),
                },
                CONTROL_DEADLINE,
                cancellation,
            )
            .await?
        {
            Reply::OwnedSession(session) => Ok(session),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn open_owned(
        &self,
        session: &OwnedSession,
        cancellation: CancellationToken,
    ) -> Result<view::Session, Error> {
        match self
            .exchange(
                Request::OpenOwned {
                    session: session.clone(),
                },
                CONTROL_DEADLINE,
                cancellation,
            )
            .await?
        {
            Reply::Session(view) => Ok(view),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn execute_owned(
        &self,
        session: &OwnedSession,
        code: &str,
        timeout: Duration,
        expect: Option<Document>,
        cancellation: CancellationToken,
    ) -> Result<view::Outcome, Error> {
        let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        if !protocol::valid_execution(timeout_ms) || !protocol::valid_code(code) {
            return Err(protocol::Refusal::InvalidRequest.into());
        }
        let request = Request::ExecuteOwned {
            session: session.clone(),
            code: code.into(),
            timeout_ms,
            expect,
        };
        match self
            .exchange(request, timeout + EXECUTION_HEADROOM, cancellation)
            .await?
        {
            Reply::Executed(outcome) => Ok(outcome),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn close_owned(
        &self,
        session: &OwnedSession,
        close_tabs: bool,
        cancellation: CancellationToken,
    ) -> Result<(), Error> {
        match self
            .exchange(
                Request::CloseOwned {
                    session: session.clone(),
                    close_tabs,
                },
                CONTROL_DEADLINE,
                cancellation,
            )
            .await?
        {
            Reply::Closed => Ok(()),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn status(&self, cancellation: CancellationToken) -> Result<view::Status, Error> {
        match self
            .exchange(Request::Status, CONTROL_DEADLINE, cancellation)
            .await?
        {
            Reply::Status(status) => Ok(status),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn open(
        &self,
        project: &str,
        cancellation: CancellationToken,
    ) -> Result<view::Session, Error> {
        let request = Request::Open {
            project: project.into(),
        };
        match self
            .exchange(request, CONTROL_DEADLINE, cancellation)
            .await?
        {
            Reply::Session(session) => Ok(session),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn update(&self, cancellation: CancellationToken) -> Result<(bool, bool), Error> {
        match self
            .exchange(Request::Update, CONTROL_DEADLINE, cancellation)
            .await?
        {
            Reply::Updated { reload, restart } => Ok((reload, restart)),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn sessions(&self, cancellation: CancellationToken) -> Result<Vec<view::Session>, Error> {
        match self
            .exchange(Request::Sessions, CONTROL_DEADLINE, cancellation)
            .await?
        {
            Reply::Sessions { sessions } => Ok(sessions),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn close(
        &self,
        project: &str,
        close_tabs: bool,
        cancellation: CancellationToken,
    ) -> Result<(), Error> {
        let request = Request::Close {
            project: project.into(),
            close_tabs,
        };
        match self
            .exchange(request, CONTROL_DEADLINE, cancellation)
            .await?
        {
            Reply::Closed => Ok(()),
            _ => Err(Error::InvalidResponse),
        }
    }

    async fn execute(
        &self,
        project: &str,
        code: &str,
        timeout: Duration,
        expect: Option<Document>,
        cancellation: CancellationToken,
    ) -> Result<view::Outcome, Error> {
        let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
        if !protocol::valid_execution(timeout_ms) || !protocol::valid_code(code) {
            return Err(crate::protocol::Refusal::InvalidRequest.into());
        }
        let request = Request::Execute {
            project: project.into(),
            code: code.into(),
            timeout_ms,
            expect,
        };
        match self
            .exchange(request, timeout + EXECUTION_HEADROOM, cancellation)
            .await?
        {
            Reply::Executed(outcome) => Ok(outcome),
            _ => Err(Error::InvalidResponse),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_requires_an_absolute_socket() {
        assert!(matches!(
            BrowserUseClient::new(PathBuf::from("control.sock")),
            Err(Error::Io(IoError::InvalidConfiguration))
        ));
        assert!(BrowserUseClient::new(std::env::temp_dir().join("control.sock")).is_ok());
    }

    #[tokio::test]
    async fn absent_host_is_unavailable_not_success() {
        let root = tempfile::tempdir().unwrap();
        let client = BrowserUseClient::new(root.path().join("control.sock")).unwrap();
        assert_eq!(
            client.status(CancellationToken::new()).await.err(),
            Some(Error::Io(IoError::Unavailable))
        );
    }

    #[tokio::test]
    async fn execution_deadline_is_validated_before_io() {
        let client = BrowserUseClient::new(std::env::temp_dir().join("absent.sock")).unwrap();
        let result = client
            .execute(
                "nds",
                "1",
                Duration::from_secs(26),
                None,
                CancellationToken::new(),
            )
            .await;
        assert_eq!(
            result.err(),
            Some(Error::Refused(protocol::Refusal::InvalidRequest))
        );
    }

    async fn reply_with(bytes: Vec<u8>) -> Result<Vec<u8>, IoError> {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("control.sock");
        let listener = socket::bind(&path).unwrap();
        let server = tokio::spawn(async move {
            let stream = socket::accept(&listener).await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut request = Vec::new();
            reader.read_until(b'\n', &mut request).await.unwrap();
            let mut stream = reader.into_inner();
            stream.write_all(&bytes).await.unwrap();
            stream.shutdown().await.unwrap();
        });
        let reply = transfer(&path, b"{\"op\":\"status\"}\n", Duration::from_secs(2)).await;
        server.await.unwrap();
        reply
    }

    #[tokio::test]
    async fn reply_buffer_survives_a_peer_half_close() {
        let payload = br#"{"result":"closed"}"#;
        let mut response = format!("{{\"bytes\":{}}}\n", payload.len()).into_bytes();
        response.extend(payload);
        assert_eq!(reply_with(response).await.unwrap(), payload);
    }

    #[tokio::test]
    async fn invalid_and_oversized_responses_are_refused_before_payload_allocation() {
        assert_eq!(
            reply_with(b"{\"bytes\":1048577}\n".to_vec()).await,
            Err(IoError::OutputLimit)
        );
        assert_eq!(
            reply_with(b"{\"bytes\":4}\nxy".to_vec()).await,
            Err(IoError::InvalidFrame)
        );
        assert_eq!(
            reply_with(b"{\"bytes\":0}".to_vec()).await,
            Err(IoError::InvalidFrame)
        );
    }

    #[tokio::test]
    async fn a_stalled_reply_reports_its_phase() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("control.sock");
        let listener = socket::bind(&path).unwrap();
        let (ready, accepted) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(async move {
            let stream = socket::accept(&listener).await.unwrap();
            let _ = ready.send(());
            let mut reader = BufReader::new(stream);
            let mut request = Vec::new();
            reader.read_until(b'\n', &mut request).await.unwrap();
            let mut tail = Vec::new();
            reader.read_to_end(&mut tail).await.unwrap();
        });
        let operation = transfer(&path, b"{\"op\":\"status\"}\n", Duration::from_millis(100));
        let (reply, ()) = tokio::join!(operation, async { accepted.await.unwrap() });
        assert_eq!(reply, Err(IoError::Timeout(Phase::Header)));
        server.await.unwrap();
    }

    #[tokio::test]
    async fn cancelling_an_accepted_exchange_closes_its_stream() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("control.sock");
        let listener = socket::bind(&path).unwrap();
        let cancellation = CancellationToken::new();
        let cancel = cancellation.clone();
        let server = tokio::spawn(async move {
            let stream = socket::accept(&listener).await.unwrap();
            let mut reader = BufReader::new(stream);
            let mut request = Vec::new();
            reader.read_until(b'\n', &mut request).await.unwrap();
            cancel.cancel();
            let mut tail = Vec::new();
            tokio::time::timeout(Duration::from_secs(2), reader.read_to_end(&mut tail))
                .await
                .expect("cancelled client must release its socket")
                .unwrap();
        });
        let client = BrowserUseClient::new(path).unwrap();
        assert_eq!(
            client.status(cancellation).await.err(),
            Some(Error::Io(IoError::Cancelled))
        );
        server.await.unwrap();
    }
}
