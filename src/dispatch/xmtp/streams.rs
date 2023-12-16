//! Turning XMTP futures into streams
use std::{future::Future, time::Duration};

use anyhow::{Context, Error, Result};
use tokio::time;
use tokio_stream::{Stream, StreamExt};
use xmtp_mls::{storage::{group_message::StoredGroupMessage, group::StoredGroup}, groups::MlsGroup};

/// SELECTS OVER GROUPS/MESSAGES AND SENDS NEW ONES 
use super::Client;

type GroupId = Vec<u8>;

pub enum NewGroupOrMessage {
    Group(StoredGroup),
    Message(StoredGroupMessage)
}

pub struct MessagesStream {
    // group and last message sent
    groups: Vec<(GroupId, i64)>,
    last_created_at: Option<i64>,
    client: Client
}

impl MessagesStream {
    // This function turns a Future into a Stream.
    // It polls the future every 100ms.
    pub fn new(client: Client) -> Self {
                
        Self { groups: Default::default(), last_created_at: Default::default(), client }
    }

    pub fn spawn(self) -> impl Stream<Item = NewGroupOrMessage> {
        let interval = time::interval(Duration::from_millis(100));

        futures::stream::unfold((future, interval), |(mut fut, mut intv)| async move {
            intv.tick().await;
            let fut_clone = fut.clone();
            match fut_clone.await {
                result => Some((result, (fut, intv))),
            }
        })
    }

    async fn get_group(client: &Client, group_id: Vec<u8>) -> Result<MlsGroup<ApiClient>> {
        client.sync_welcomes().await?;
        let group = client.group(group_id)?;
        group.sync().await.context("failed to sync group")?;

        Ok(group)
    }

    async fn groups(client: &Client) -> Result<Vec<StoredGroup>> {
        let groups = client.find_groups
    }
}

