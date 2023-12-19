use std::collections::HashMap;

use xmtp_mls::storage::group_message::StoredGroupMessage;

use crate::types::Group;

pub type GroupId = Vec<u8>;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Message {
    pub user: String,
    pub text: String,
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
        messages.iter().cloned().map(|Message { user, text }| (user, text)).unzip()
    }

    pub fn add(&mut self, id: &GroupId, mut message: Message) {
        let lines = message.text.lines().count();

        message.user = format!("{}:", message.user);
        (0..lines).for_each(|_| message.user.push('\n'));
        let messages = self.get_or_insert(id);
        messages.push(message);
    }

    pub fn add_group_messages(&mut self, map: HashMap<GroupId, Vec<StoredGroupMessage>>) {
        // log::debug!("Adding Messages {:#?}", map);
        let extension = map
            .into_iter()
            .map(|(id, msgs)| (id, msgs.into_iter().map(From::from).collect::<Vec<_>>()));
        for (group, messages) in extension {
            if let Some(msgs) = self.inner.get_mut(&group) {
                msgs.extend(messages);
            } else {
                // this should not happen
                self.inner.insert(group, messages);
            }
        }
    }

    pub fn add_groups(&mut self, groups: Vec<Group>) {
        let groups =
            groups.into_iter().filter(|g| !self.inner.contains_key(&g.id)).collect::<Vec<_>>();
        self.inner.extend(groups.into_iter().map(|g| (g.id, Vec::new())));
    }
}

impl From<StoredGroupMessage> for Message {
    fn from(group_message: StoredGroupMessage) -> Message {
        let text = String::from_utf8_lossy(&group_message.decrypted_message_bytes);
        let user = group_message.sender_account_address;
        let user = format!(
            "{}...{} ",
            user.get(0..4).unwrap(),
            user.get(user.len() - 4..user.len()).unwrap()
        );
        Message { user, text: text.to_string() }
    }
}
