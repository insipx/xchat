use argh::FromArgs;

#[derive(FromArgs)]
#[allow(dead_code)]
/// XMTP CLI Chat Application
pub struct XChatApp {
    /// the Identity of the User
    #[argh(option)]
    pub wallet: Option<String>,

    /// use xchat against a local XMTP deployment
    #[argh(switch)]
    pub local: bool,
}
