//! Turning XMTP futures into streams
use std::{collections::HashMap, sync::Arc, time::Duration};

use anyhow::{Context, Error, Result};
use futures::executor::block_on;
// use chrono::offset::{Local, TimeZone};
use tokio::{sync::Mutex, time};
use tokio_stream::Stream;
use xmtp_mls::{groups::MlsGroup, storage::group_message::StoredGroupMessage};

use super::xmtp_async::Client;
use crate::types::Group;

// type ClientBuilder = xmtp_mls::builder::ClientBuilder<ApiClient, LocalWallet>;

pub enum NewGroupsOrMessages {
    Groups(Vec<Group>),
    Messages(HashMap<Group, Vec<StoredGroupMessage>>),
    None,
}

pub enum NewGroupOrMessage {
    Group(Group),
    Message(StoredGroupMessage)
}

/// Stream of new Groups or Messages
#[derive(Debug, Clone)]
pub struct MessagesStream {
    // group and last message sent
    groups: Vec<Group>,
    // the time the last group was created
    last_created_at: Option<i64>,
    client: Arc<Mutex<Client>>,
}

impl MessagesStream {
    // This function turns a Future into a Stream.
    // It polls the future every 50ms.
    pub fn new(client: Arc<Mutex<Client>>) -> Self {
        Self { groups: Default::default(), last_created_at: Default::default(), client }
    }

    pub fn spawn(self) -> impl Stream<Item = NewGroupsOrMessages> {
        let interval = time::interval(Duration::from_millis(100));

        futures::stream::unfold((self, interval), |(mut state, mut intv)| async move {
            intv.tick().await;

            match state.poll_xmtp().await {
                Ok(res) => Some((res, (state, intv))),
                Err(e) => {
                    log::error!("{e}");
                    Some((NewGroupsOrMessages::None, (state, intv)))
                }
            }
        })
    }

    async fn poll_xmtp(&mut self) -> Result<NewGroupsOrMessages> {
        let groups = self.all_groups();
        let messages = self.all_new_messages();

        let now = std::time::Instant::now();
        let (groups, messages) = tokio::try_join!(groups, messages)?;
        let after = std::time::Instant::now();
        if (after - now) > std::time::Duration::from_millis(50) {
            log::debug!("Took {:?} to get new groups/messages", after - now);
        }

        if !groups.is_empty() {
            self.groups.extend(groups.iter().cloned());
            self.update_last_created_at();
            return Ok(NewGroupsOrMessages::Groups(groups));
        }

        if !Self::check_if_empty(&messages) {
            self.update_last_sent_at(&messages);
            log::debug!("Receiving {} new messages", messages.len());
            let messages: HashMap<Group, Vec<StoredGroupMessage>> =
                HashMap::from_iter(messages.into_values());
            let messages: HashMap<Group, Vec<StoredGroupMessage>> =
                messages.into_iter().filter(|(_, m)| m.len() > 0).collect();
            if messages.len() > 0 {
                return Ok(NewGroupsOrMessages::Messages(messages));
            }
        }
        Ok(NewGroupsOrMessages::None)
    }

    // get any new groups and sync them
    async fn all_groups(&self) -> Result<Vec<Group>> {
        let client = self.client.clone();
        let last_created_at = self.last_created_at.clone();

        tokio::task::spawn_blocking(move || {
            let client = futures::executor::block_on(client.lock());
            Self::groups(&*client, last_created_at)
        })
        .await?
    }

    async fn all_new_messages(&self) -> Result<HashMap<Vec<u8>, (Group, Vec<StoredGroupMessage>)>> {
        let client = self.client.clone();
        let groups = self.groups.clone();
        Ok(tokio::task::spawn_blocking(move || {
            let client = block_on(client.lock());
            let mut map = HashMap::new();
            for mut group in groups {
                let messages = Self::messages(&client, &group)?;
                if messages.len() > 0 {
                    if let Some(msg) = messages.last() {
                        group.last_sent_at = msg.sent_at_ns;
                    }
                    map.insert(group.id.clone(), (group, messages));
                }
            }
            Ok::<_, Error>(map)
        })
        .await??)
    }

    // #[tracing::instrument(name = "message", skip(client, group))]
    fn messages(client: &Client, group: &Group) -> Result<Vec<StoredGroupMessage>> {
        let Group { id, created_at, last_sent_at, .. } = group.clone();
        let group = MlsGroup::new(client, id, created_at);
        let now = std::time::Instant::now();
        futures::executor::block_on(group.sync()).context("Failed to sync group for messages")?;
        let after = std::time::Instant::now();
        if (after - now) > std::time::Duration::from_millis(100) {
            log::debug!("Took {:?} to sync group", after - now);
        }
        Ok(group.find_messages(None, None, Some(last_sent_at), None)?)
    }

    // #[tracing::instrument(name = "groups", skip(client))]
    fn groups(client: &Client, last_created_at: Option<i64>) -> Result<Vec<Group>> {
        let now = std::time::Instant::now();
        block_on(client.sync_welcomes())?;
        let after = std::time::Instant::now();
        if (after - now) > std::time::Duration::from_millis(100) {
            log::debug!("Took {:?} to sync welcomes", after - now);
        }
        let groups = client
            .find_groups(None, last_created_at, None, None)
            .context("Could not find groups")?;

        let groups: Vec<_> = groups.into_iter().map(Group::from).collect();
        if groups.len() > 0 {
            log::debug!("Found groups {:?}", groups);
        }
        Ok::<_, Error>(groups)
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

    fn update_last_sent_at(
        &mut self,
        messages: &HashMap<Vec<u8>, (Group, Vec<StoredGroupMessage>)>,
    ) {
        for group in &mut self.groups {
            if let Some((g, _)) = messages.get(&group.id) {
                group.last_sent_at = g.last_sent_at;
            }
        }
    }

    fn check_if_empty(messages: &HashMap<Vec<u8>, (Group, Vec<StoredGroupMessage>)>) -> bool {
        messages.values().all(|(_, msgs)| msgs.is_empty())
    }
}
