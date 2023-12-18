//! Commands events which may manipulate the state of the terminal
use anyhow::Result;
use tokio::{
    sync::{
        broadcast::{error::SendError, Sender as BroadcastSender},
        mpsc::{Receiver, Sender},
    },
    task::JoinHandle,
};

use crate::dispatch::{Action, XMTPAction};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandAction {
    Help,
    /// Register a new identity with XMTP
    Register,
    /// Generate a new ephemeral wallet identity
    Generate,
    /// Create a new group
    Create,
    /// Join a group
    Join,
    /// Invite to a group
    Invite,
    /// Information about you (Wallet Address, ENS Profile, etc.)
    Me,
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
    tx: BroadcastSender<Action>,
    xmtp: Sender<XMTPAction>,
    commands: Receiver<CommandAction>,
}

impl From<String> for CommandAction {
    fn from(s: String) -> CommandAction {
        match s.as_str() {
            "help" => CommandAction::Help,
            "quit" => CommandAction::Quit,
            "register" => CommandAction::Register,
            "list" => CommandAction::List(ListCommand::Users),
            "generate" => CommandAction::Generate,
            "create" => CommandAction::Create,
            "join" => CommandAction::Join,
            "invite" => CommandAction::Invite,
            "me" => CommandAction::Me,
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
        msg.push_str("\n    /generate: generate a new ephemeral wallet identity");
        msg.push_str("\n    /create: create a new group");
        msg.push_str("\n    /join {group_id}: join a group");
        msg.push_str("\n    /invite {user_id}: invite to join a group");
        msg.push_str(
            "\n    /me: get information about the current sessions wallet address, balance, network, etc. ",
        );
        msg
    }
}

impl Commands {
    pub fn new(
        tx: BroadcastSender<Action>,
        xmtp: Sender<XMTPAction>,
        commands: Receiver<CommandAction>,
    ) -> Self {
        Self { tx, xmtp, commands }
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(event) = self.commands.recv().await {
                let res: Result<usize> = match event {
                    CommandAction::Help => self.send_message(CommandAction::help()),
                    CommandAction::Quit => self.tx.send(Action::Quit).map_err(Into::into),
                    CommandAction::Register => Ok(0),
                    CommandAction::Generate => Ok(0),
                    CommandAction::List(_) => Ok(0),
                    CommandAction::Create => {
                        log::debug!("Sent CreateGroup XMTP Action");
                        self.xmtp
                            .send(XMTPAction::CreateGroup)
                            .await
                            .map(|_| 0usize)
                            .map_err(Into::into)
                    }
                    CommandAction::Join => Ok(0),
                    CommandAction::Invite => Ok(0),
                    CommandAction::Me => Ok(0),
                    CommandAction::Unknown(s) => self.send_message(format!(
                        "Unknown command: /{}. use `/help` to get a list of commands",
                        s
                    )),
                };
                if let Err(e) = res {
                    log::error!("Help message failed to send {}", e);
                }
            }
        })
    }

    pub fn send_message(&mut self, msg: String) -> Result<usize> {
        self.tx.send(Action::ReceiveMessage(vec![0], ("xchat".into(), msg)))?;
        Ok(0)
    }
}
