//! Commands events which may manipulate the state of the terminal
use tokio::{
    sync::{
        broadcast::{error::SendError, Sender},
        mpsc::Receiver,
    },
    task::JoinHandle,
};
use tokio_stream::StreamExt;

use crate::dispatch::Action;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    Help,
    Register,
    Quit,
    List(ListCommand),
    Unknown(String),
}

impl From<CommandAction> for Action {
    fn from(action: CommandAction) -> Action {
        Action::Command(action)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListCommand {
    Group,
    Users,
}

impl From<ListCommand> for CommandAction {
    fn from(list: ListCommand) -> CommandAction {
        CommandAction::List(list)
    }
}

pub struct Commands {
    tx: Sender<Action>,
    commands: Receiver<CommandAction>,
}

impl From<String> for CommandAction {
    fn from(s: String) -> CommandAction {
        match s.as_str() {
            "help" => CommandAction::Help,
            "quit" => CommandAction::Quit,
            "register" => CommandAction::Register,
            "list" => CommandAction::List(ListCommand::Users),
            s => CommandAction::Unknown(s.into()),
        }
    }
}

impl CommandAction {
    /// Return a help message for these commands
    fn help() -> String {
        let mut msg = String::from("xChat Help Message");
        msg.push_str("\n    /help: Receive this help dialogue");
        msg.push_str("\n    /quit: quit the app");
        msg.push_str("\n    /register: register this instance with XMTP");
        msg.push_str("\n    /list {groups|users}: list the users or groups you are apart of");
        msg
    }
}

impl Commands {
    pub fn new(tx: Sender<Action>, commands: Receiver<CommandAction>) -> Self {
        Self { tx, commands }
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(event) = self.commands.recv().await {
                let res = match event {
                    CommandAction::Help => self.send_message(CommandAction::help()),
                    CommandAction::Quit => self.tx.send(Action::Quit),
                    CommandAction::Register => Ok(0),
                    CommandAction::List(list) => Ok(0),
                    CommandAction::Unknown(s) => self.send_message(format!(
                        "Unknown command: /{}. use `/help` to get a list of commands",
                        s
                    )),
                };
                if let Err(e) = res {
                    log::error!("Help message failed to send");
                }
            }
        })
    }

    pub fn send_message(&mut self, msg: String) -> Result<usize, SendError<Action>> {
        self.tx.send(Action::ReceiveMessage(vec![0], ("xchat".into(), msg)))
    }
}
