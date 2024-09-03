//! Async Interface to libXMTP

use std::{fmt, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Context as _, Result};
use ethers::signers::{LocalWallet, Signer};
use rand::{rngs::StdRng, SeedableRng};
use tokio_stream::{Stream, StreamExt};
use xmtp_api_grpc::grpc_api_helper::Client as ApiClient;
use xmtp_id::associations::{generate_inbox_id, RecoverableEcdsaSignature};
use xmtp_mls::{
    identity::IdentityStrategy,
    groups::MlsGroup,
    storage::{group_message::StoredGroupMessage, EncryptedMessageStore, StorageOption},
    InboxOwner,
    client::ClientError
};
use xmtp_mls::groups::GroupMetadataOptions;
use crate::{cli::XChatApp, types::Group};

pub type Client = xmtp_mls::client::Client<ApiClient>;
type ClientBuilder = xmtp_mls::builder::ClientBuilder<ApiClient>;

impl Group {
    pub fn into_mls(self, client: &Client) -> MlsGroup {
        MlsGroup::new(client.context().clone(), self.id, self.created_at)
    }
}

impl From<MlsGroup> for Group {
    fn from(group: MlsGroup) -> Group {
        Group::new(group.group_id, group.created_at_ns, 0)
    }
}

impl From<&MlsGroup> for Group {
    fn from(group: &MlsGroup) -> Group {
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

        let nonce = 0;
        let inbox_id = generate_inbox_id(&wallet.get_address(), &nonce);
        let strategy = IdentityStrategy::CreateIfNotFound(inbox_id, wallet.get_address(), nonce, None);

        let client = Self::create_client(
            &opts,
            db.clone(),
            strategy,
        )
        .await?;

        let identity = client.identity();
        let mut signature_request = identity.signature_request().expect("cant be none");
        let signature = RecoverableEcdsaSignature::new(
            signature_request.signature_text(),
            wallet.sign(signature_request.signature_text().as_str())
                .unwrap()
                .into(),
        );
        signature_request
            .add_signature(Box::new(signature))
            .await
            .unwrap();
            let res = client.register_identity(signature_request).await?;
        log::debug!("--------------------------- res: {:?}", res);
        Ok(Self { wallet, db, client: Arc::new(client) })
    }

    async fn create_client(
        opts: &XChatApp,
        db: PathBuf,
        account: IdentityStrategy,
    ) -> Result<Client> {
        let msg_store = Self::get_encrypted_store(db)?;
        let mut builder = ClientBuilder::new(account).store(msg_store);

        if opts.local {
            builder = builder
                .api_client(ApiClient::create("http://localhost:5556".into(), false).await?);
        } else {
            builder = builder
                .api_client(ApiClient::create("https://dev.xmtp.network:5556".into(), true).await?);
        }

        Ok(builder.build().await?)
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

    pub async fn messages(&self) -> Result<impl Stream<Item = Result<StoredGroupMessage, ClientError>>> {
        Ok(Client::stream_all_messages(self.client.clone()).await?)
    }

    // #[tracing::instrument(name = "send_message", skip(self, to))]
    pub async fn send_message(&self, to: Group, msg: String) -> Result<()> {
        let now = std::time::Instant::now();
        let group = to.into_mls(&self.client);
        let msg = msg.into_bytes();
        group.send_message(msg.as_slice(), &self.client).await?;
        let after = std::time::Instant::now();
        log::debug!("Took {:?} to send message", after - now);
        Ok(())
    }

    pub async fn create_group(&self) -> Result<Group> {
        let client = self.client.clone();
        let group = client.create_group(None, GroupMetadataOptions::default())?;
        Ok(group.into())
    }

    pub async fn invite_user(&self, group: Group, user: String) -> Result<()> {
        let group = group.into_mls(&self.client);
        group.add_members(&self.client, vec![user]).await?;
        Ok(())
    }

    pub async fn installation_public_key(&self) -> Vec<u8> {
        self.client.installation_public_key()
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
