//! The byte lease remains held until the consumer finishes the write, including a blocked writer.
use super::link::Ended;
use std::sync::Arc;
use tokio::sync::{OwnedSemaphorePermit, Semaphore, mpsc, watch};

#[derive(Clone)]
pub(super) struct Terminal(pub watch::Sender<Option<Ended>>);

impl Terminal {
    pub fn fail(&self, reason: Ended) {
        self.0.send_if_modified(|value| {
            if value.is_some() {
                return false;
            }
            *value = Some(reason);
            true
        });
    }
    pub fn ended(&self) -> bool {
        self.0.borrow().is_some()
    }
}

pub(super) struct Packet<T> {
    pub value: T,
    pub charge: Arc<OwnedSemaphorePermit>,
}

pub(super) struct Outbox<T> {
    send: mpsc::Sender<Packet<T>>,
    bytes: Arc<Semaphore>,
    terminal: Terminal,
}

impl<T> Outbox<T> {
    pub fn new(
        messages: usize,
        bytes: u32,
        terminal: Terminal,
    ) -> (Self, mpsc::Receiver<Packet<T>>) {
        let (send, receive) = mpsc::channel(messages);
        (
            Self {
                send,
                bytes: Arc::new(Semaphore::new(bytes as usize)),
                terminal,
            },
            receive,
        )
    }

    pub fn send(&self, value: T, bytes: usize) -> bool {
        if self.terminal.ended() {
            return false;
        }
        let Some(permit) = u32::try_from(bytes)
            .ok()
            .and_then(|count| self.bytes.clone().try_acquire_many_owned(count).ok())
        else {
            self.terminal.fail(Ended::Capacity);
            return false;
        };
        match self.send.try_send(Packet {
            value,
            charge: Arc::new(permit),
        }) {
            Ok(()) => true,
            Err(mpsc::error::TrySendError::Full(_)) => {
                self.terminal.fail(Ended::Capacity);
                false
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                self.terminal.fail(Ended::Transport);
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn active_writes_hold_the_byte_budget_and_failure_is_terminal() {
        let (sender, terminal) = watch::channel(None);
        let (queue, mut reader) = Outbox::new(2, 8, Terminal(sender));
        assert!(queue.send("fixture", 6));
        let packet = reader.recv().await.unwrap();
        assert!(!queue.send("fixture", 3));
        assert_eq!(*terminal.borrow(), Some(Ended::Capacity));
        drop(packet);
        assert_eq!(queue.bytes.available_permits(), 8);
        assert!(!queue.send("fixture", 1));
    }

    #[tokio::test]
    async fn a_full_message_queue_releases_its_rejected_byte_lease() {
        let (sender, terminal) = watch::channel(None);
        let (queue, mut reader) = Outbox::new(1, 8, Terminal(sender));
        assert!(queue.send("fixture", 2));
        assert!(!queue.send("fixture", 2));
        assert_eq!(queue.bytes.available_permits(), 6);
        assert_eq!(*terminal.borrow(), Some(Ended::Capacity));
        drop(reader.recv().await.unwrap());
        assert_eq!(queue.bytes.available_permits(), 8);
    }

    #[test]
    fn the_first_terminal_reason_is_not_replaced_by_cleanup_errors() {
        let (sender, receiver) = watch::channel(None);
        let terminal = Terminal(sender);
        terminal.fail(Ended::Capacity);
        terminal.fail(Ended::Transport);
        assert_eq!(*receiver.borrow(), Some(Ended::Capacity));
    }
}
