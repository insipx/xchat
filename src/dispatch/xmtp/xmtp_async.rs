//! Async Interface to libXMTP

use std::{path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context, Result};
use ethers::signers::{LocalWallet, Signer};
use rand::{rngs::StdRng, SeedableRng};
use tokio::{sync::Mutex, task::JoinError};
use tokio_stream::Stream;
use xmtp_api_grpc::grpc_api_helper::Client as ApiClient;
use xmtp_mls::{
    builder::IdentityStrategy,
    groups::MlsGroup,
    storage::{EncryptedMessageStore, StorageOption},
    Network,
};

use crate::{
    cli::XChatApp,
    dispatch::xmtp::streams::{MessagesStream, NewGroupsOrMessages},
    types::Group,
};

pub type Client = xmtp_mls::client::Client<ApiClient>;
type ClientBuilder = xmtp_mls::builder::ClientBuilder<ApiClient, LocalWallet>;

impl Group {
    pub fn into_mls(self, client: &Client) -> MlsGroup<ApiClient> {
        MlsGroup::new(client, self.id, self.created_at)
    }
}

impl<A> From<MlsGroup<'_, A>> for Group {
    fn from(group: MlsGroup<'_, A>) -> Group {
        Group::new(group.group_id, group.created_at_ns, 0)
    }
}

impl<A> From<&MlsGroup<'_, A>> for Group {
    fn from(group: &MlsGroup<'_, A>) -> Group {
        Group::new(group.group_id.clone(), group.created_at_ns, 0)
    }
}

/*
enum WalletType {
    /// A Locally generated wallet for this instance of xChat
    Ephemeral(LocalWallet),
    External(WalletConnect)
}
*/

#[allow(unused)]
#[derive(Debug, Clone)]
pub struct AsyncXmtp {
    pub wallet: LocalWallet,
    pub db: PathBuf,
    client: Arc<Mutex<Client>>,
}

impl AsyncXmtp {
    pub async fn new_ephemeral(opts: XChatApp) -> Result<Self> {
        let wallet = LocalWallet::new(&mut StdRng::from_entropy());
        let db_name = format!("{}-db.sqlite", hex::encode(wallet.address()));
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

        Ok(Self { wallet, db, client: Arc::new(Mutex::new(client)) })
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

    /// Get a stream of new messages and groups
    pub fn messages(&self) -> impl Stream<Item = NewGroupsOrMessages> {
        MessagesStream::new(self.client.clone()).spawn()
    }

    // #[tracing::instrument(name = "send_message", skip(self, to))]
    pub async fn send_message(&self, to: Group, msg: String) -> Result<()> {
        let client = self.client.lock().await;
        let now = std::time::Instant::now();
        let group = to.into_mls(&client);
        let msg = msg.into_bytes();
        group.send_message(msg.as_slice()).await?;
        let after = std::time::Instant::now();
        log::debug!("Took {:?} to send message", after - now);
        Ok(())
    }

    pub async fn create_group(&self) -> Result<Group> {
        let client = self.client.clone();
        let client = client.lock().await;
        let group = client.create_group()?;
        Ok(group.into())
    }

    pub async fn invite_user(&self, group: Group, user: String) -> Result<()> {
        let client = self.client.clone();
        let client = client.lock().await;
        let group = group.into_mls(&client);
        group.add_members(vec![user]).await?;
        Ok(())
    }

    pub async fn installation_public_key(&self) -> Vec<u8> {
        let client = self.client.lock().await;
        client.installation_public_key()
    }
}

impl Drop for AsyncXmtp {
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

#[allow(dead_code)]
fn unwrap_join<T>(res: Result<T, JoinError>) -> T {
    match res {
        Ok(v) => v,
        Err(e) => {
            log::error!("XMTP Task failed {}", e);
            panic!("oh no");
        }
    }
}
