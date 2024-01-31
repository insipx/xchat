use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GroupIdWrapper(pub Vec<u8>);

impl fmt::Display for GroupIdWrapper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let first_2 = &self.0[0..2];
        let len = self.0.len() - 3;
        let last_2 = &self.0[len..];
        write!(f, "{}{}...{}{}", first_2[0], first_2[1], last_2[0], last_2[1])
    }
}

impl From<Vec<u8>> for GroupIdWrapper {
    fn from(value: Vec<u8>) -> Self {
        GroupIdWrapper(value)
    }
}
pub type GroupId = Vec<u8>;

// can form a group by calling `MlsGroup::new()` and passing our client reference
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct Group {
    pub id: GroupId,
    // timestamp this group was created at. Used for reconstructing MlsGroup.
    pub created_at: i64,
    // last message sent in this group
    pub last_sent_at: i64,
    is_fake: bool,
}

impl Group {
    pub fn new(id: GroupId, created_at: i64, last_sent_at: i64) -> Self {
        Self { id, created_at, last_sent_at, is_fake: false }
    }

    pub fn new_fake(id: u8) -> Self {
        Group { id: vec![id], created_at: 0, last_sent_at: 0, is_fake: true }
    }

    pub fn is_fake(&self) -> bool {
        self.is_fake
    }
}
