//! The XCHat Terminal Event Handler
//! This event handler streams events from the terminal and converts them into an [`Action`]

use crossterm::event::{Event, EventStream};
use tokio::{sync::broadcast::Sender, task::JoinHandle};
use tokio_stream::StreamExt;

use crate::dispatch::Action;

pub struct Events {
    tx: Sender<Action>,
}

impl Events {
    pub fn new(tx: Sender<Action>) -> Self {
        Self { tx }
    }

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            let mut events = EventStream::new();
            while let Some(event) = events.next().await {
                if event.is_err() {
                    log::error!("Event Error, e={}", event.unwrap_err());
                    continue;
                }

                let res = match event.expect("Checked Error") {
                    Event::Key(key_event) => self.tx.send(Action::KeyPress(key_event)),
                    Event::Resize(x, y) => self.tx.send(Action::Resize(x, y)).map_err(Into::into),
                    _ => continue,
                };

                if let Err(e) = res {
                    log::error!("Error sending events {}. They need to be re-tried", e);
                }
            }
        })
    }
}
