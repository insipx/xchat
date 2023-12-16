//! Events to process with libxmtp
mod streams;

use std::path::PathBuf;

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
use xmtp_api_grpc::grpc_api_helper::Client as ApiClient;
use xmtp_mls::{
    builder::IdentityStrategy,
    storage::{EncryptedMessageStore, StorageOption},
    Network,
};

use super::Action;
use crate::cli::XChatApp;

type Client = xmtp_mls::client::Client<ApiClient>;
type ClientBuilder = xmtp_mls::builder::ClientBuilder<ApiClient, LocalWallet>;

/// Actions for XMTP
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XMTPAction {
    /// Send a group message
    SendMessage(String),
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
    rx: Receiver<XMTPAction>,
    wallet: LocalWallet,
    db: PathBuf,
    client: Client,
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

        Ok(Self { tx, rx, wallet, db, client, opts })
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            while let Some(event) = self.rx.recv().await {
                let res: Result<usize, SendError<Action>> = match event {
                    XMTPAction::SendMessage(_) => Ok(0),
                };

                if let Err(e) = res {
                    log::error!("Action failed to send {}", e);
                }
            }
        })
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
