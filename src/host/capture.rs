//! Recorder images are retained here, under the same live policy boundary as the journal.
use super::{Host, PendingWork, Session, SessionLife, token};
use crate::diagnostics::CollectionEpoch;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use nddev_browseruse_contract::{CaptureFailure as Error, CaptureReply as Reply, CaptureRequest};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::Write,
    path::{Component, Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
    time::Instant,
};

pub(super) struct FrameScope {
    generation: Arc<AtomicU64>,
    children: HashSet<String>,
    unknown: bool,
}
impl Default for FrameScope {
    fn default() -> Self {
        Self {
            generation: Arc::new(AtomicU64::new(0)),
            children: HashSet::new(),
            unknown: false,
        }
    }
}

impl FrameScope {
    pub fn observe(&mut self, method: &str, params: &Value) {
        if matches!(
            method,
            "Page.frameNavigated" | "Page.frameAttached" | "Page.frameDetached"
        ) {
            self.advance();
        }
        if let Some(id) = params["sessionId"].as_str() {
            if method == "Target.attachedToTarget" && params["targetInfo"]["type"] == "iframe" {
                self.advance();
                if self.children.len() < 4096 {
                    self.children.insert(id.into());
                } else {
                    self.unknown = true;
                }
            } else if method == "Target.detachedFromTarget" && self.children.remove(id) {
                self.advance();
            }
        }
    }
    pub fn advance(&self) {
        self.generation.fetch_add(1, Ordering::AcqRel);
    }
}
impl Drop for FrameScope {
    fn drop(&mut self) {
        self.advance();
    }
}

struct PageScope {
    nonce: String,
    frame: Arc<AtomicU64>,
    generation: u64,
}

pub(super) struct Capture {
    id: String,
    directory: PathBuf,
    root: PathBuf,
    epoch: CollectionEpoch,
    life: Arc<SessionLife>,
    active: bool,
    started: Instant,
    pages: HashMap<u32, PageScope>,
    bytes: u64,
    frames: u64,
}

pub(super) fn revoke(session: &mut Session) {
    if let Some(capture) = session.capture.take() {
        capture
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .active = false;
    }
    session.recording = None;
}

fn directory(root: &Path, requested: Option<&str>, id: &str) -> Option<PathBuf> {
    let path = requested
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("recordings").join(id));
    let relative = if path.is_absolute() {
        path.strip_prefix(root).ok()?
    } else {
        &path
    };
    let parts: Vec<_> = relative.components().collect();
    if parts.is_empty()
        || parts.len() > 8
        || parts
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return None;
    }
    Some(root.join(relative))
}

fn private_tree(tree: &Value, sites: &[String]) -> bool {
    let mut pending = vec![tree];
    let mut visited = 0;
    while let Some(tree) = pending.pop() {
        visited += 1;
        if visited > 4096 {
            return true;
        }
        let Some(url) = tree["frame"]["url"].as_str() else {
            return true;
        };
        let host = crate::activity::clean::host(url).to_ascii_lowercase();
        if sites.iter().any(|site| {
            host == *site
                || host
                    .strip_suffix(site.as_str())
                    .is_some_and(|prefix| prefix.ends_with('.'))
        }) {
            return true;
        }
        if let Some(children) = tree["childFrames"].as_array() {
            pending.extend(children);
        }
    }
    false
}

fn write(root: &Path, directory: &Path, at: u64, page: u32, bytes: &[u8]) -> std::io::Result<()> {
    let mut path = root.to_owned();
    for part in directory
        .strip_prefix(root)
        .map_err(|_| std::io::ErrorKind::InvalidInput)?
        .components()
    {
        let Component::Normal(part) = part else {
            return Err(std::io::ErrorKind::InvalidInput.into());
        };
        path.push(part);
        crate::runtime::private_directory(&path)
            .map_err(|_| std::io::ErrorKind::PermissionDenied)?;
    }
    path.push("frames");
    crate::runtime::private_directory(&path).map_err(|_| std::io::ErrorKind::PermissionDenied)?;
    let name = format!("{at:08}-p{page}");
    let temporary = path.join(format!(".{name}.tmp"));
    let final_path = path.join(format!("{name}.jpg"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        std::fs::hard_link(&temporary, &final_path)
    })();
    drop(file);
    let _ = std::fs::remove_file(&temporary);
    result
}

impl Host {
    pub(super) async fn capture(&self, project: &str, client: u64, params: &Value) -> Value {
        let limit = self.config.executor_settings.recorder.frame_bytes;
        if !crate::protocol::json_fits(params, limit.div_ceil(3) * 4 + 8192)
            || params["data"]
                .as_str()
                .is_some_and(|data| data.len() > limit.div_ceil(3) * 4)
        {
            return serde_json::to_value(Reply::refused(Error::FrameLimit)).unwrap_or_default();
        }
        let reply = match serde_json::from_value(params.clone()) {
            Ok(CaptureRequest::Begin {
                id,
                directory,
                epoch,
            }) => self.capture_begin(project, &id, directory.as_deref(), &epoch),
            Ok(CaptureRequest::Page {
                id,
                page,
                target,
                epoch,
            }) => {
                self.capture_page(project, client, &id, page, &target, &epoch)
                    .await
            }
            Ok(CaptureRequest::Frame {
                id,
                scope,
                page,
                at,
                data,
                epoch,
            }) => {
                self.capture_frame(project, &id, &scope, page, at, &data, &epoch)
                    .await
            }
            Ok(CaptureRequest::Stop { id }) => {
                let mut state = self.state();
                match state.sessions.get_mut(project) {
                    Some(session) => {
                        let matches = session.capture.as_ref().is_some_and(|capture| {
                            capture.lock().unwrap_or_else(|error| error.into_inner()).id == id
                        });
                        if matches {
                            revoke(session);
                            Reply::kept()
                        } else {
                            Reply::refused(Error::Stopped)
                        }
                    }
                    None => Reply::refused(Error::TargetClosed),
                }
            }
            Err(_) => Reply::refused(Error::InvalidRequest),
        };
        serde_json::to_value(reply).unwrap_or_default()
    }

    fn capture_begin(
        &self,
        project: &str,
        id: &str,
        requested: Option<&str>,
        epoch: &str,
    ) -> Reply {
        let Some(stamp) = self
            .config
            .diagnostics
            .content_epoch()
            .filter(|stamp| stamp.token() == epoch)
        else {
            return Reply::refused(Error::PolicyChanged);
        };
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Reply::refused(Error::InvalidRequest);
        }
        let mut state = self.state();
        if !self.config.diagnostics.accepts_content(stamp) {
            return Reply::refused(Error::PolicyChanged);
        }
        let Some(session) = state
            .sessions
            .get_mut(project)
            .filter(|s| !s.closing && !s.life.stop.is_cancelled())
        else {
            return Reply::refused(Error::TargetClosed);
        };
        if session.capture.as_ref().is_some_and(|capture| {
            capture
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .active
        }) {
            return Reply::refused(Error::Capacity);
        }
        let Some(directory) = directory(&session.artifacts, requested, id) else {
            return Reply::refused(Error::InvalidRequest);
        };
        let mut reply = Reply::kept();
        reply.directory = Some(directory.to_string_lossy().into_owned());
        session.capture = Some(Arc::new(Mutex::new(Capture {
            id: id.into(),
            directory,
            root: session.artifacts.clone(),
            epoch: stamp,
            life: session.life.clone(),
            active: true,
            started: Instant::now(),
            pages: HashMap::new(),
            bytes: 0,
            frames: 0,
        })));
        reply
    }

    async fn capture_page(
        &self,
        project: &str,
        client: u64,
        id: &str,
        page: u32,
        target: &str,
        epoch: &str,
    ) -> Reply {
        if page == 0 {
            return Reply::refused(Error::InvalidRequest);
        }
        let (capture, tab, frame, generation, sites, children) = {
            let state = self.state();
            let Some(capture) = state
                .sessions
                .get(project)
                .and_then(|session| session.capture.clone())
            else {
                return Reply::refused(Error::Stopped);
            };
            let Some((tab, entry)) = state
                .tabs
                .iter()
                .find(|(_, tab)| tab.project == project && tab.target == target)
            else {
                return Reply::refused(Error::TargetClosed);
            };
            let children = if state.private_sites.is_empty() {
                Vec::new()
            } else {
                if entry.frames.unknown
                    || entry.frames.children.len()
                        > self.config.executor_settings.recorder.frame_targets
                {
                    return Reply::refused(Error::Capacity);
                }
                entry.frames.children.iter().cloned().collect()
            };
            (
                capture,
                *tab,
                entry.frames.generation.clone(),
                entry.frames.generation.load(Ordering::Acquire),
                state.private_sites.clone(),
                children,
            )
        };
        let mut private = false;
        // OOPIF documents have their own Page domain and can be absent from the parent's tree.
        for child in std::iter::once(None).chain(children.into_iter().map(Some)) {
            let mut command = json!({ "type": "cdp", "group": project, "tab": tab, "method": "Page.getFrameTree" });
            if let Some(child) = child {
                command["session"] = child.into();
            }
            let result = self.session_request(project, client, command).await;
            if frame.load(Ordering::Acquire) != generation {
                return Reply::refused(Error::DocumentChanged);
            }
            let Ok(result) = result else {
                return Reply::refused(Error::Unavailable);
            };
            if result.get("frameTree").is_none() || private_tree(&result["frameTree"], &sites) {
                private = true;
                break;
            }
        }
        let mut capture = capture.lock().unwrap_or_else(|error| error.into_inner());
        if !capture.active || capture.id != id || capture.life.stop.is_cancelled() {
            return Reply::refused(Error::Stopped);
        }
        if capture.epoch.token() != epoch || !self.config.diagnostics.accepts_content(capture.epoch)
        {
            return Reply::refused(Error::PolicyChanged);
        }
        if frame.load(Ordering::Acquire) != generation {
            return Reply::refused(Error::DocumentChanged);
        }
        if private {
            capture.pages.remove(&page);
            return Reply::refused(Error::PrivatePage);
        }
        if capture.pages.len() >= crate::protocol::MAX_TABS && !capture.pages.contains_key(&page) {
            return Reply::refused(Error::Capacity);
        }
        let Ok(nonce) = token() else {
            return Reply::refused(Error::Unavailable);
        };
        capture.pages.insert(
            page,
            PageScope {
                nonce: nonce.clone(),
                frame,
                generation,
            },
        );
        let mut reply = Reply::kept();
        reply.scope = Some(nonce);
        reply
    }

    #[allow(clippy::too_many_arguments)]
    async fn capture_frame(
        &self,
        project: &str,
        id: &str,
        scope: &str,
        page: u32,
        at: u64,
        data: &str,
        epoch: &str,
    ) -> Reply {
        let capture = self
            .state()
            .sessions
            .get(project)
            .and_then(|session| session.capture.clone());
        let Some(capture) = capture else {
            return Reply::refused(Error::Stopped);
        };
        let Ok(bytes) = STANDARD.decode(data) else {
            return Reply::refused(Error::InvalidRequest);
        };
        let limits = self.config.executor_settings.recorder.clone();
        if bytes.len() > limits.frame_bytes
            || !bytes.starts_with(&[0xff, 0xd8])
            || !bytes.ends_with(&[0xff, 0xd9])
        {
            return Reply::refused(Error::FrameLimit);
        }
        let policy = self.config.diagnostics.clone();
        let (id, scope, epoch) = (id.to_owned(), scope.to_owned(), epoch.to_owned());
        let mut work = PendingWork::new(
            capture
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .life
                .clone(),
        );
        tokio::task::spawn_blocking(move || {
            let mut capture = capture.lock().unwrap_or_else(|error| error.into_inner());
            let reply = (|| {
                if !capture.active || capture.id != id || capture.life.stop.is_cancelled() {
                    return Reply::refused(Error::Stopped);
                }
                if capture.epoch.token() != epoch || !policy.accepts_content(capture.epoch) {
                    return Reply::refused(Error::PolicyChanged);
                }
                let Some(source) = capture.pages.get(&page) else {
                    return Reply::refused(Error::DocumentChanged);
                };
                if source.nonce != scope
                    || source.frame.load(Ordering::Acquire) != source.generation
                {
                    return Reply::refused(Error::DocumentChanged);
                }
                if at > limits.max_ms
                    || capture.started.elapsed().as_millis() > limits.max_ms.into()
                    || capture.bytes.saturating_add(bytes.len() as u64) > limits.max_bytes
                {
                    capture.active = false;
                    return Reply::refused(Error::Capacity);
                }
                capture.bytes += bytes.len() as u64;
                if write(&capture.root, &capture.directory, at, page, &bytes).is_err() {
                    return Reply::refused(Error::Unavailable);
                }
                capture.frames += 1;
                let mut reply = Reply::kept();
                reply.frames = capture.frames;
                reply.bytes = capture.bytes;
                reply
            })();
            work.settle();
            reply
        })
        .await
        .unwrap_or_else(|_| Reply::refused(Error::Unavailable))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capture_paths_stay_below_the_sessions_artifacts() {
        let root = std::env::temp_dir().join("capture-path-fixture");
        assert_eq!(
            directory(&root, None, "fixture"),
            Some(root.join("recordings/fixture"))
        );
        for path in ["../outside", ".", "a/../../outside", "a/./../outside"] {
            assert!(directory(&root, Some(path), "fixture").is_none());
        }
        let outside = root.with_file_name("outside");
        assert!(directory(&root, outside.to_str(), "fixture").is_none());
        assert_eq!(
            directory(&root, Some("custom/fixture"), "fixture"),
            Some(root.join("custom/fixture"))
        );
    }

    #[test]
    fn a_private_child_frame_denies_the_whole_image() {
        let tree = json!({ "frame": { "url": "https://public.test/" }, "childFrames": [
            { "frame": { "url": "https://nested.secret.test/" } }
        ] });
        assert!(private_tree(&tree, &["secret.test".into()]));
        assert!(!private_tree(&tree, &["other.test".into()]));
        assert!(!private_tree(
            &json!({ "frame": { "url": "https://notsecret.test/" } }),
            &["secret.test".into()]
        ));
    }

    #[test]
    fn navigating_or_detaching_a_tab_invalidates_its_existing_image_scope() {
        let scope = FrameScope::default();
        let generation = scope.generation.load(Ordering::Acquire);
        scope.advance();
        assert_ne!(scope.generation.load(Ordering::Acquire), generation);
        let counter = scope.generation.clone();
        let generation = counter.load(Ordering::Acquire);
        drop(scope);
        assert_ne!(counter.load(Ordering::Acquire), generation);
    }

    #[test]
    fn images_are_complete_private_files_and_never_replace_an_existing_image() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("recordings/fixture");
        let bytes = [0xff, 0xd8, 1, 0xff, 0xd9];
        write(root.path(), &directory, 10, 1, &bytes).unwrap();
        assert!(write(root.path(), &directory, 10, 1, &[0]).is_err());
        let path = directory.join("frames/00000010-p1.jpg");
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        assert_eq!(
            std::fs::read_dir(directory.join("frames")).unwrap().count(),
            1
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
}
