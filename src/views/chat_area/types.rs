use std::collections::HashMap;

use xmtp_mls::storage::group_message::{GroupMessageKind, StoredGroupMessage};
use xmtp_proto::xmtp::message_contents::EncodedContent;
use prost::Message as _;

use crate::types::Group;

pub type GroupId = Vec<u8>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub user: String,
    pub kind: GroupMessageKind,
    // timestamp of message in nano-seconds
    pub sent_at: i64,
    pub text: String,
}

impl Default for Message {
    fn default() -> Self {
        Message {
            user: Default::default(),
            kind: GroupMessageKind::Application,
            text: Default::default(),
            sent_at: 0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Messages {
    pub inner: HashMap<GroupId, Vec<Message>>,
    pub focused: GroupId,
}

pub const WELCOME_MESSAGE: &str = std::include_str!("../../../static/welcome_message.txt");

impl Messages {
    pub fn set_focus(&mut self, id: &GroupId) {
        self.focused = id.clone();
    }

    pub fn get_or_insert(&mut self, id: &GroupId) -> &mut Vec<Message> {
        if !self.inner.contains_key(id) {
            self.inner.insert(id.clone(), Default::default());
        }

        self.inner.get_mut(id).expect("Checked for existence")
    }

    pub fn get(&self) -> (Vec<String>, Vec<String>) {
        let messages = &self.inner.get(&self.focused).expect("Focused group id must always exist");
        messages
            .iter()
            .cloned()
            .filter_map(|Message { user, text, kind, .. }| {
                if kind == GroupMessageKind::Application {
                    Some((user, text))
                } else {
                    None
                }
            })
            .unzip()
    }

    pub fn add(&mut self, id: &GroupId, mut message: Message) {
        let lines = message.text.lines().count();

        message.user = format!("{}:", message.user);
        (0..lines).for_each(|_| message.user.push('\n'));
        let messages = self.get_or_insert(id);
        messages.push(message);
    }

    pub fn add_group_message(&mut self, message: StoredGroupMessage) {
        let group_id = message.group_id.clone();
        if let Some(msgs) = self.inner.get_mut(&group_id) {
            msgs.push(Message::from(message));
        } else {
            self.inner.insert(group_id, vec![Message::from(message)]);
        }
    }

    pub fn add_group_messages(&mut self, map: HashMap<GroupId, Vec<StoredGroupMessage>>) {
        // log::debug!("Adding Messages {:#?}", map);
        let extension = map.into_iter().map(|(id, msgs)| {
            (
                id,
                msgs.into_iter()
                    .map(Message::from)
                    .filter(|m| matches!(m.kind, GroupMessageKind::Application))
                    .collect::<Vec<_>>(),
            )
        });

        for (group, messages) in extension {
            if let Some(msgs) = self.inner.get_mut(&group) {
                msgs.extend(messages);
            } else {
                // this should not happen
                self.inner.insert(group, messages);
            }
        }
        // log::debug!("Messages {:#?}", self.inner.values().collect::<Vec<_>>());
    }

    pub fn add_groups(&mut self, groups: Vec<Group>) {
        let groups =
            groups.into_iter().filter(|g| !self.inner.contains_key(&g.id)).collect::<Vec<_>>();
        self.inner.extend(groups.into_iter().map(|g| (g.id, Vec::new())));
    }
}

impl From<StoredGroupMessage> for Message {
    fn from(group_message: StoredGroupMessage) -> Message {
        let content = EncodedContent::decode(group_message.decrypted_message_bytes.as_slice());
        let msg = content.unwrap();
        let text = String::from_utf8_lossy(msg.content.as_slice());
        let user = group_message.sender_inbox_id;
        let user = format!(
            "{}...{} ",
            user.get(0..4)
                .expect("Sender account address MUST be at least 20 characters; qed")
                .to_string(),
            user.get((user.len() - 4)..(user.len()))
                .expect("Sender account address MUST be at least 20 characters; qed")
                .to_string()
        );
        Message {
            user,
            text: text.to_string(),
            kind: group_message.kind,
            sent_at: group_message.sent_at_ns,
        }
    }
}
