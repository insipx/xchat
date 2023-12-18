//! Turning XMTP futures into streams
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Error, Result};
use futures::{future::Future, stream};
use tokio::time;
use tokio_stream::{Stream, StreamExt};
use xmtp_mls::{
    groups::MlsGroup,
    storage::{group::StoredGroup, group_message::StoredGroupMessage},
};

/// SELECTS OVER GROUPS/MESSAGES AND SENDS NEW ONES
use super::Client;
use crate::types::Group;

type GroupId = Vec<u8>;

pub enum NewGroupsOrMessages {
    Groups(Vec<Group>),
    Messages(HashMap<Group, Vec<StoredGroupMessage>>),
    None,
}

/// Stream of new Groups or Messages
pub struct MessagesStream {
    // group and last message sent
    groups: HashSet<Group>,
    // the time the last group was created
    last_created_at: Option<i64>,
    client: Arc<Client>,
}

impl MessagesStream {
    // This function turns a Future into a Stream.
    // It polls the future every 100ms.
    pub fn new(client: Arc<Client>) -> Self {
        Self { groups: Default::default(), last_created_at: Default::default(), client }
    }

    pub fn spawn(self) -> impl Stream<Item = NewGroupsOrMessages> {
        let interval = time::interval(Duration::from_millis(100));

        futures::stream::unfold((self, interval), |(mut state, mut intv)| async move {
            intv.tick().await;

            match state.poll_xmtp().await {
                Ok(res) => Some((res, (state, intv))),
                Err(e) => {
                    log::error!("Error polling xmtp for messages {e}");
                    Some((NewGroupsOrMessages::None, (state, intv)))
                }
            }
        })
    }

    async fn poll_xmtp(&mut self) -> Result<NewGroupsOrMessages> {
        let groups = self.groups().await?;

        if !groups.is_empty() {
            self.groups.extend(groups.iter().cloned());
            self.update_last_created_at();
            return Ok(NewGroupsOrMessages::Groups(groups));
        }

        let messages = self.all_new_messages().await?;
        if !messages.is_empty() {
            self.update_last_sent_at(&messages);
            return Ok(NewGroupsOrMessages::Messages(messages));
        }
        Ok(NewGroupsOrMessages::None)
    }

    // get any new groups and sync them
    async fn groups(&self) -> Result<Vec<Group>> {
        let client = self.client.clone();
        let last_created_at = self.last_created_at.clone();
        Ok(tokio::task::spawn_blocking(move || {
            futures::executor::block_on(client.sync_welcomes())?;
            let groups = client
                .find_groups(None, last_created_at, None, None)
                .context("Could not find groups")?;
            /*
                        for group in &groups {
                            futures::executor::block_on(group.sync()).context("Failed to sync group")?;
                        }
            */
            let groups =
                groups.into_iter().map(|g| Group::new(g.group_id, g.created_at_ns, 0)).collect();

            Ok::<_, Error>(groups)
        })
        .await??)
    }

    async fn all_new_messages(&self) -> Result<HashMap<Group, Vec<StoredGroupMessage>>> {
        let client = self.client.clone();
        let groups = self.groups.clone();
        Ok(tokio::task::spawn_blocking(move || {
            let mut map = HashMap::new();
            for mut group in groups {
                let messages = Self::messages(&client, &group)?;
                if let Some(msg) = messages.last() {
                    group.last_sent_at = msg.sent_at_ns;
                }
                map.insert(group.clone(), messages);
            }
            Ok::<_, Error>(map)
        })
        .await??)
    }

    fn messages(client: &Client, group: &Group) -> Result<Vec<StoredGroupMessage>> {
        let Group { id, created_at, last_sent_at, .. } = group.clone();
        let group = MlsGroup::new(client, id, created_at);
        futures::executor::block_on(group.sync()).context("Failed to sync group for messages")?;
        Ok(group.find_messages(None, None, Some(last_sent_at), None)?)
    }

    /// Update the time that the last group was created at
    fn update_last_created_at(&mut self) {
        let mut last_created_at = 0;
        let mut last_created = self.groups.iter().map(|g| g.created_at);

        while let Some(created) = last_created.next() {
            last_created_at = std::cmp::max(last_created_at, created);
        }

        self.last_created_at = Some(last_created_at);
    }

    fn update_last_sent_at(&mut self, messages: &HashMap<Group, Vec<StoredGroupMessage>>) {
        for group in messages.keys() {
            let _ = self.groups.replace(group.clone());
        }
    }
}
