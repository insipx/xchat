use argh::FromArgs;

#[derive(FromArgs)]
/// XMTP CLI Chat Application
pub struct XChatApp {
    /// the Identity of the User
    #[argh(option)]
    wallet: Option<String>,
}
