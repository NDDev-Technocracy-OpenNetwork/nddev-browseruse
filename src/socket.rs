//! Shared platform socket adapter. No process or global endpoint is installed here.
pub(crate) use nddev_browseruse_local_transport::{Listener, Stream, accept, answers, bind};
