//! Commands events which may manipulate the state of the terminal
use anyhow::{bail, Result};
use tokio::{
    sync::{
        broadcast::Sender as BroadcastSender,
        mpsc::{Receiver, Sender},
    },
    task::JoinHandle,
};

use crate::{
    dispatch::{Action, XMTPAction},
    types::Group,
};

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
    Invite(Group, String),
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

    pub fn from_string(command: String, group: &Group) -> Result<Self> {
        let command = command.split(" ").collect::<Vec<_>>();
        let cmd = match command[0] {
            "help" => CommandAction::Help,
            "quit" => CommandAction::Quit,
            "register" => CommandAction::Register,
            "list" => CommandAction::List(ListCommand::Users),
            "generate" => CommandAction::Generate,
            "create" => CommandAction::Create,
            "join" => CommandAction::Join,
            "invite" => {
                if command.get(1).is_some() {
                    CommandAction::Invite(group.clone(), command[1].into())
                } else {
                    bail!("`/invite` requires indicating the wallet address of the user to invite");
                }
            }
            "me" => CommandAction::Me,
            s => CommandAction::Unknown(s.into()),
        };
        Ok(cmd)
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

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            match self.event_loop().await {
                Ok(v) => v,
                Err(e) => log::error!("command failed {}", e),
            }
        })
    }

    async fn event_loop(mut self) -> Result<()> {
        while let Some(event) = self.commands.recv().await {
            match event {
                CommandAction::Help => self.send_message(CommandAction::help()).map(|_| ())?,
                CommandAction::Quit => self.tx.send(Action::Quit).map(|_| ())?,
                CommandAction::Register => (),
                CommandAction::Generate => (),
                CommandAction::List(_) => (),
                CommandAction::Create => {
                    log::debug!("Sent CreateGroup XMTP Action");
                    self.xmtp.send(XMTPAction::CreateGroup).await?;
                }
                CommandAction::Join => (),
                CommandAction::Invite(group, user) => {
                    log::debug!("Inviting to group");
                    self.xmtp.send(XMTPAction::Invite(group, user)).await?;
                }
                CommandAction::Me => self.xmtp.send(XMTPAction::Info).await?,
                CommandAction::Unknown(s) => self.send_message(format!(
                    "Unknown command: /{}. use `/help` to get a list of commands",
                    s
                ))?,
            };
        }
        Ok(())
    }

    pub fn send_message(&mut self, msg: String) -> Result<()> {
        self.tx.send(Action::ReceiveMessage(vec![0], ("xchat".into(), msg)))?;
        Ok(())
    }
}
