//! Cooperative cancellation shared by a browser operation and its owner.

use tokio::sync::watch;

#[derive(Clone, Debug)]
pub struct CancellationToken {
    cancelled: watch::Sender<bool>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: watch::channel(false).0,
        }
    }

    pub fn cancel(&self) {
        self.cancelled.send_replace(true);
    }

    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    pub async fn cancelled(&self) {
        let mut receiver = self.cancelled.subscribe();
        // wait_for checks the current value too, including cancellation before subscription.
        let _ = receiver.wait_for(|cancelled| *cancelled).await;
    }
}
