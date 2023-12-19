//! Events to process with libxmtp
mod streams;
pub mod xmtp_async;

use anyhow::Result;
use ethers::signers::Signer;
use tokio::{
    sync::{broadcast::Sender, mpsc::Receiver},
    task::JoinHandle,
};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use self::streams::NewGroupsOrMessages;
use super::Action;
use crate::{cli::XChatApp, dispatch::xmtp::xmtp_async::AsyncXmtp, types::Group};

/// Actions for XMTP
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XMTPAction {
    /// Send message to (group_id, message)
    SendMessage(Group, String),
    CreateGroup,
    /// Invite user to a group
    /// only admin/group creator can do this
    Invite(Group, String),
    /// Send information about the current user
    Info,
}

impl From<XMTPAction> for Action {
    fn from(action: XMTPAction) -> Action {
        Action::XMTP(action)
    }
}

pub struct XMTP {
    tx: Sender<Action>,
    rx: ReceiverStream<XMTPAction>,
    xmtp: AsyncXmtp,
}

impl XMTP {
    pub async fn new(tx: Sender<Action>, rx: Receiver<XMTPAction>, opts: XChatApp) -> Result<Self> {
        let xmtp = AsyncXmtp::new_ephemeral(opts).await?;
        Ok(Self { tx, rx: ReceiverStream::new(rx), xmtp })
    }

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            match self.event_loop().await {
                Ok(_) => (),
                Err(e) => log::error!("error running XMTP Events {}", e),
            }
        })
    }

    async fn event_loop(mut self) -> Result<()> {
        log::info!("Spawning handle");
        let events_stream = &mut self.rx;
        let messages_stream = self.xmtp.messages();
        tokio::pin!(messages_stream);

        loop {
            tokio::select! {
                msg_or_group = messages_stream.next() => {
                    let action = match msg_or_group {
                        Some(NewGroupsOrMessages::Groups(groups)) => Action::NewGroups(groups),
                        Some(NewGroupsOrMessages::Messages(msgs)) => Action::ReceiveMessages(msgs.into_iter().map(|(g, msgs)| { (g.id, msgs)}).collect()),
                        Some(NewGroupsOrMessages::None) => Action::Noop,
                        None => Action::Noop,
                    };
                    match action {
                        a @ Action::NewGroups(_) | a @ Action::ReceiveMessages(_) => self.tx.send(a).map(|_| ()),
                        _ => Ok(()),
                    }?;
                },
                event = events_stream.next() => {
                    let res: Result<()> = match event {
                        Some(XMTPAction::SendMessage(group, m)) => {
                            if group.is_fake() {
                                self.tx.send(Action::ReceiveMessage(group.id, ("xchat".into(), "Invalid Buffer, cannot send MLS messages to this buffer.".into())))?;
                                continue;
                            }
                            self.xmtp.send_message(group, m).await
                        },
                        Some(XMTPAction::CreateGroup) => {
                            log::debug!("Creating MLS group");
                            let _ = self.xmtp.create_group().await?;
                            Ok(())
                        },
                        Some(XMTPAction::Invite(group, user)) => {
                            self.xmtp.invite_user(group, user).await?;
                            Ok(())
                        },
                        Some(XMTPAction::Info) => {
                            let mut info_message = format!("-------------- Information --------------");
                            info_message.push_str(&format!("\nWallet Address: {}", hex::encode(self.xmtp.wallet.address())));
                            info_message.push_str(&format!("\nDatabase: {}", self.xmtp.db.to_str().unwrap_or("not displayable (not utf8?)")));
                            info_message.push_str(&format!("\nInstallation Public Key: {}", hex::encode(self.xmtp.installation_public_key())));
                            self.tx.send(Action::ReceiveMessage(vec![0], ("xchat".into(), info_message)))?;
                            Ok(())
                        }
                        None => Ok(())
                    };
                    if let Err(e) = res {
                        log::error!("Action failed to send {}", e);
                    }
                }
            };
        }
    }
}
