use std::collections::HashMap;

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
    pub fn new() -> Self {
        Default::default()
    }

    pub fn focused(&self) -> &GroupId {
        &self.focused
    }

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
}
