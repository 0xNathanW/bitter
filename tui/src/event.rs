use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use tokio::{sync::{mpsc, oneshot}, task::JoinHandle};

pub type EventRx = mpsc::UnboundedReceiver<Event>;
pub type EventTx = mpsc::UnboundedSender<Event>;

pub fn spawn_events() -> (JoinHandle<()>, EventRx, oneshot::Sender<()>) {
    let (tx, rx) = mpsc::unbounded_channel();
    let (sd_tx, sd_rx) = oneshot::channel::<()>();

    let handle = tokio::spawn(async move {
        let mut reader = EventStream::new();
        tokio::select! {
            Some(event) = reader.next() => {
                match event {
                    Ok(event) => {
                        tx.send(event).unwrap();
                    },
                    Err(_) => {}
                }
            }

            _ = sd_rx => {
                return;
            }
        }
    });

    (handle, rx, sd_tx)
}
