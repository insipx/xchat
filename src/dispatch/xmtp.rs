//! Events to process with libxmtp
mod streams;

use std::{path::PathBuf, sync::Arc};

use anyhow::{anyhow, bail, Context, Error, Result};
use directories::ProjectDirs;
use ethers::signers::{LocalWallet, Signer};
use rand::{rngs::StdRng, SeedableRng};
use tokio::{
    sync::{
        broadcast::{error::SendError, Sender},
        mpsc::Receiver,
    },
    task::JoinHandle,
};
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use xmtp_api_grpc::grpc_api_helper::Client as ApiClient;
use xmtp_mls::{
    builder::IdentityStrategy,
    groups::MlsGroup,
    storage::{EncryptedMessageStore, StorageOption},
    Network,
};

use self::streams::{MessagesStream, NewGroupsOrMessages};
use super::Action;
use crate::{
    cli::XChatApp,
    types::{Client, Group, GroupId},
};

type ClientBuilder = xmtp_mls::builder::ClientBuilder<ApiClient, LocalWallet>;

/// Actions for XMTP
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XMTPAction {
    /// Send message to (group_id, message)
    SendMessage(Group, String),
    CreateGroup,
}

impl From<XMTPAction> for Action {
    fn from(action: XMTPAction) -> Action {
        Action::XMTP(action)
    }
}

/*
enum WalletType {
    /// A Locally generated wallet for this instance of xChat
    Ephemeral(LocalWallet),
    External(WalletConnect)
}
*/

pub struct XMTP {
    tx: Sender<Action>,
    rx: ReceiverStream<XMTPAction>,
    wallet: LocalWallet,
    db: PathBuf,
    client: Arc<Client>,
    opts: XChatApp,
}

impl XMTP {
    pub async fn new(tx: Sender<Action>, rx: Receiver<XMTPAction>, opts: XChatApp) -> Result<Self> {
        let wallet = LocalWallet::new(&mut StdRng::from_entropy());
        let db_name = format!("{}-db", hex::encode(wallet.address()));
        let mut db = crate::util::project_directory()
            .ok_or(anyhow!("User does not have a valid home directory"))?
            .data_local_dir()
            .to_path_buf();
        db.push(db_name);
        let client = Self::create_client(
            &opts,
            db.clone(),
            IdentityStrategy::CreateIfNotFound(wallet.clone()),
        )
        .await?;

        client.register_identity().await.context("Initialization Failed")?;

        Ok(Self { tx, rx: ReceiverStream::new(rx), wallet, db, client: Arc::new(client), opts })
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            log::info!("Spawning handle");
            let events_stream = &mut self.rx;
            let messages_stream = MessagesStream::new(self.client.clone()).spawn();
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
                        let _ = match action {
                            a @ Action::NewGroups(_) | a @ Action::ReceiveMessages(_) => self.tx.send(a),
                            _ => Ok(0),
                        };
                    },
                    event = events_stream.next() => {
                        let res: Result<usize> = match event {
                            Some(XMTPAction::SendMessage(group, m)) => {
                                if group.is_fake() {
                                    let _ = self.tx.send(Action::ReceiveMessage(group.id, ("xchat".into(), "Invalid Buffer, cannot send MLS messages to this buffer.".into())));
                                    continue;
                                }
                                let client = self.client.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    let group = group.into_mls(&client);
                                    futures::executor::block_on(group.send_message(m.into_bytes().as_slice())).unwrap();
                                }).await;
                                Ok(0)
                            },
                            Some(XMTPAction::CreateGroup) => {
                                log::debug!("Creating MLS group");
                                Self::handle_create_group(&self.client, self.tx.clone())
                            },
                            None => Ok(0)
                        };
                        if let Err(e) = res {
                            log::error!("Action failed to send {}", e);
                        }
                    }
                };
            }
        })
    }

    fn handle_create_group(client: &Client, tx: Sender<Action>) -> Result<usize> {
        let group = client.create_group()?;
        tx.send(Action::NewGroups(vec![group.into()]))?;
        Ok(0)
    }

    async fn create_client(
        opts: &XChatApp,
        db: PathBuf,
        account: IdentityStrategy<LocalWallet>,
    ) -> Result<Client> {
        let msg_store = Self::get_encrypted_store(db)?;
        let mut builder = ClientBuilder::new(account).store(msg_store);

        if opts.local {
            builder = builder.network(Network::Local("http://localhost:5556")).api_client(
                ApiClient::create("http://localhost:5556".into(), false).await.unwrap(),
            );
        } else {
            builder = builder.network(Network::Dev).api_client(
                ApiClient::create("https://dev.xmtp.network:5556".into(), true).await.unwrap(),
            );
        }

        Ok(builder.build()?)
    }

    fn get_encrypted_store(db: PathBuf) -> Result<EncryptedMessageStore> {
        let s = db.to_string_lossy().to_string();
        let store = EncryptedMessageStore::new_unencrypted(StorageOption::Persistent(s))
            .context("Persistent message store could not be opened.")?;
        Ok(store)
    }
}

impl Drop for XMTP {
    fn drop(&mut self) {
        log::info!("DROPPING");
        use std::io::ErrorKind;
        //TODO: Check if wallet type is ephemeral; if so delete it
        if let Err(e) = std::fs::remove_file(self.db.clone()) {
            match e.kind() {
                // if for some reason there is no db file we don't care anyway
                ErrorKind::NotFound => (),
                _ => log::error!("DB File could not be removed {}", e),
            }
        }
    }
}
