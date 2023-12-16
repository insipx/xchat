//! Commands which manipulate the state of the terminal
use tokio::sync::mpsc::Receiver;

use crate::dispatch::Action;

pub struct Commands {
    events: Receiver<Action>,
}
