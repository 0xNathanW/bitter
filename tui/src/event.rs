use crossterm::event::{Event, EventStream};
use futures::StreamExt;
use tokio::{sync::mpsc, task::JoinHandle};

pub type EventRx = mpsc::UnboundedReceiver<Event>;
pub type EventTx = mpsc::UnboundedSender<Event>;

pub fn spawn_events() -> (JoinHandle<()>, EventRx) {
    let (tx, rx) = mpsc::unbounded_channel();

    let handle = tokio::spawn(async move {
        let mut reader = EventStream::new();
        while let Some(event) = reader.next().await {
            match event {
                Ok(event) => {
                    tx.send(event).unwrap();
                },
                Err(_) => {
                    continue;
                }
            }
        }
    });
    (handle, rx)
}
