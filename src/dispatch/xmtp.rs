//! Events to process with libxmtp
pub mod xmtp_async;

use anyhow::Result;
use ethers::signers::Signer;
use tokio::{
    sync::{broadcast::Sender, mpsc::Receiver},
    task::JoinHandle,
};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

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
    opts: XChatApp,
}

impl XMTP {
    pub fn new(tx: Sender<Action>, rx: Receiver<XMTPAction>, opts: XChatApp) -> Self {
        Self { tx, rx: ReceiverStream::new(rx), opts }
    }

    pub fn spawn(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            match self.event_loop().await {
                Ok(_) => (),
                Err(e) => log::error!("error running XMTP Events {}", e),
            }
        })
    }

    async fn event_loop(self) -> Result<()> {
        log::info!("Spawning handle");
        let XMTP { tx, mut rx, opts } = self;

        let xmtp = AsyncXmtp::new_ephemeral(opts).await?;
        let messages = xmtp.messages().await?;
        futures::pin_mut!(messages);
        let conversations = xmtp.subscribe_conversations().await?;
        futures::pin_mut!(conversations);

        let events = &mut rx;
        loop {
            tokio::select! {
                Some(msg) = messages.next() => {
                    tx.send(Action::ReceiveMessage(msg?))?;
                },
                Some(group) = conversations.next() => {
                    let group = group.unwrap();
                    log::debug!("Following conversation for group {:?}", group.id);
                    tx.send(Action::NewGroups(vec![group]))?;
                },
                event = events.next() => {
                    let res: Result<()> = match event {
                        Some(XMTPAction::SendMessage(group, m)) => {
                            if group.is_fake() {
                                tx.send(Action::FakeMessage(group.id, ("xchat".into(), "Invalid Buffer, cannot send MLS messages to this buffer.".into())))?;
                                continue;
                            }
                            xmtp.send_message(group, m).await
                        },
                        Some(XMTPAction::CreateGroup) => {
                            log::debug!("Creating MLS group");
                            let group = xmtp.create_group().await?;
                            tx.send(Action::NewGroups(vec![group]))?;
                            Ok(())
                        },
                        Some(XMTPAction::Invite(group, user)) => {
                            let user = if !user.starts_with("0x") { format!("0x{}", user) } else { user };
                            xmtp.invite_user(group, user).await?;
                            Ok(())
                        },
                        Some(XMTPAction::Info) => {
                            Self::welcome_message(&tx, &xmtp).await
                        }
                        None => Ok(())
                    };
                    if let Err(e) = res {
                        log::debug!("Action failed to send {}", e);
                    }
                }
            };
        }
    }

    async fn welcome_message(tx: &Sender<Action>, xmtp: &AsyncXmtp) -> Result<()> {
        let mut info_message = format!("-------------- Information --------------");
        info_message
            .push_str(&format!("\nWallet Address: 0x{}", hex::encode(xmtp.wallet.address())));
        info_message.push_str(&format!(
            "\nDatabase: {}",
            xmtp.db.to_str().unwrap_or("not displayable (not utf8?)")
        ));
        info_message.push_str(&format!(
            "\nInstallation Public Key: {}",
            hex::encode(xmtp.installation_public_key().await)
        ));
        tx.send(Action::FakeMessage(vec![0], ("xchat".into(), info_message)))?;
        Ok(())
    }
}
