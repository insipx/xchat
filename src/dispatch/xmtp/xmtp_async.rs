//! Async Interface to libXMTP

use std::{collections::HashSet, fmt, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context as _, Error, Result};
use ethers::signers::{LocalWallet, Signer};
use rand::{rngs::StdRng, SeedableRng};
use tokio::{sync::mpsc, task::JoinError};
use tokio_stream::{wrappers::UnboundedReceiverStream, Stream, StreamExt};
use xmtp_api_grpc::grpc_api_helper::Client as ApiClient;
use xmtp_mls::{
    builder::IdentityStrategy,
    groups::MlsGroup,
    storage::{group_message::StoredGroupMessage, EncryptedMessageStore, StorageOption},
    Network,
};

use crate::{cli::XChatApp, types::Group};

pub type Client = xmtp_mls::client::Client<ApiClient>;
type ClientBuilder = xmtp_mls::builder::ClientBuilder<ApiClient, LocalWallet>;

impl Group {
    pub fn into_mls<'a>(self, client: &'a Client) -> MlsGroup<'a, ApiClient> {
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
pub struct AsyncXmtp {
    pub wallet: LocalWallet,
    pub db: PathBuf,
    pub client: Arc<Client>,
}

impl fmt::Debug for AsyncXmtp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AsyncXmtp")
            .field("wallet", &self.wallet)
            .field("db", &self.db)
            .field("client", &self.client)
            .finish()
    }
}

impl AsyncXmtp {
    /// Generate a xmtp client from a random seed
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

        Ok(Self { wallet, db, client: Arc::new(client) })
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

    pub async fn subscribe_conversations(&self) -> Result<impl Stream<Item = Group> + '_> {
        let stream = self.client.stream_conversations().await?.map(Group::from);
        Ok(stream)
    }

    pub async fn messages(&self) -> Result<AsyncMessagesStream> {
        let stream = AsyncMessagesStream::new(self.client.clone());
        Ok(stream)
    }

    // #[tracing::instrument(name = "send_message", skip(self, to))]
    pub async fn send_message(&self, to: Group, msg: String) -> Result<()> {
        let now = std::time::Instant::now();
        let group = to.into_mls(&self.client);
        let msg = msg.into_bytes();
        group.send_message(msg.as_slice()).await?;
        let after = std::time::Instant::now();
        log::debug!("Took {:?} to send message", after - now);
        Ok(())
    }

    pub async fn create_group(&self) -> Result<Group> {
        let client = self.client.clone();
        tokio::task::spawn_blocking(move || {
            let group = client.create_group()?;
            Ok(group.into())
        })
        .await?
    }

    pub async fn invite_user(&self, group: Group, user: String) -> Result<()> {
        let group = group.into_mls(&self.client);
        group.add_members(vec![user]).await?;
        Ok(())
    }

    pub async fn installation_public_key(&self) -> Vec<u8> {
        self.client.installation_public_key()
    }

    pub async fn all_groups(&self) -> Result<Vec<Group>> {
        self.client.sync_welcomes().await?;
        let groups: Vec<Group> = self
            .client
            .find_groups(None, None, None, None)
            .context("Could not find groups")?
            .into_iter()
            .map(Group::from)
            .collect();
        Ok(groups)
    }
}

pub struct AsyncMessagesStream {
    client: Arc<Client>,
    groups: HashSet<Group>, // list of followed conversations
    rx: Option<mpsc::UnboundedReceiver<StoredGroupMessage>>,
    tx: mpsc::UnboundedSender<StoredGroupMessage>,
    handles: Vec<tokio::task::JoinHandle<Result<()>>>,
}

impl AsyncMessagesStream {
    pub fn new(client: Arc<Client>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self { client, groups: HashSet::new(), tx, rx: Some(rx), handles: Vec::new() }
    }

    /// Refresh with known groups
    pub fn refresh(&mut self, groups: Vec<Group>) -> Result<()> {
        for group in groups {
            if self.groups.insert(group.clone()) {
                self.follow_conversation(group)?;
            }
        }
        Ok(())
    }

    /// spawn a task that for each group to the messages of a stream from a group into one channel
    pub fn follow_conversation(&mut self, group: Group) -> Result<()> {
        if self.groups.insert(group.clone()) {
            log::debug!("Following conversation {:?}", group);
            let (client, tx) = (self.client.clone(), self.tx.clone());
            let handle = tokio::spawn(async move {
                log::debug!("Spawning to follow converstation {:?}", group);
                let stream = group.into_mls(&client);
                let mut stream = stream.stream().await?;
                while let Some(msg) = stream.next().await {
                    tx.send(msg)?;
                }
                Ok::<_, Error>(())
            });
            self.handles.push(handle);
        }
        Ok(())
    }

    /// Can only call this once
    pub fn stream(&mut self) -> Option<impl Stream<Item = StoredGroupMessage>> {
        let rx = self.rx.take();
        rx.map(|r| UnboundedReceiverStream::new(r))
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
