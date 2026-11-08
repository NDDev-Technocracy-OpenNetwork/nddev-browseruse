use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Private local operation data. Never implement Debug or send these fields to diagnostics.
#[derive(Deserialize, ToSchema)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum CaptureRequest {
    Begin {
        id: String,
        directory: Option<String>,
        epoch: String,
    },
    Page {
        id: String,
        page: u32,
        target: String,
        epoch: String,
    },
    Frame {
        id: String,
        scope: String,
        page: u32,
        at: u64,
        data: String,
        epoch: String,
    },
    Stop {
        id: String,
    },
}

#[derive(Clone, Copy, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CaptureFailure {
    PolicyChanged,
    DocumentChanged,
    PrivatePage,
    Capacity,
    InvalidRequest,
    Unavailable,
    Stopped,
    FrameLimit,
    TargetClosed,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CaptureReply {
    pub kept: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<CaptureFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub directory: Option<String>,
    pub frames: u64,
    pub bytes: u64,
}

impl CaptureReply {
    pub fn refused(error: CaptureFailure) -> Self {
        Self {
            kept: false,
            error: Some(error),
            scope: None,
            directory: None,
            frames: 0,
            bytes: 0,
        }
    }
    pub fn kept() -> Self {
        Self {
            kept: true,
            error: None,
            scope: None,
            directory: None,
            frames: 0,
            bytes: 0,
        }
    }
}
