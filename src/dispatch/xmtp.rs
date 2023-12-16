use super::Action;

/// Actions for XMTP
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XMTPAction {
    /// Send a group message
    SendMessage(String),
}

impl From<XMTPAction> for Action {
    fn from(action: XMTPAction) -> Action {
        Action::XMTP(action)
    }
}
