mod cli;
mod dispatch;
mod events;
mod pages;
mod types;
mod util;
mod views;
mod xmtp;

use std::io::stderr;

use anyhow::Result;
use crossterm::{
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use ratatui::prelude::*;
use tokio::sync::{
    broadcast::{self, Receiver},
    mpsc,
};

use crate::{
    dispatch::{Action, Dispatcher, PageRender, Store},
    events::Events,
    pages::ChatPage,
};

type CrosstermTerminal = Terminal<CrosstermBackend<std::io::Stderr>>;

#[tokio::main]
async fn main() -> Result<()> {
    // console_subscriber::init();
    self::util::init_logging()?;
    #[allow(unused)]
    let app: cli::XChatApp = argh::from_env();

    enable_raw_mode().unwrap();
    stderr().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stderr()))?;

    let (tx, _) = broadcast::channel::<Action>(100);

    let (xmtp_tx, xmtp_rx) = mpsc::channel(500);

    let handle = Events::new(tx.clone()).spawn();
    let chat_page = ChatPage::new(xmtp_tx);

    render_loop(&mut terminal, tx.subscribe(), chat_page, 1_000.0, 120.0).await;

    handle.abort();
    disable_raw_mode().unwrap();
    stderr().execute(LeaveAlternateScreen)?;
    Ok(())
}

pub async fn render_loop(
    terminal: &mut CrosstermTerminal,
    mut events: Receiver<Action>,
    mut chat_page: ChatPage,
    tick_rate: f64,
    frame_rate: f64,
) {
    let tick_delay = std::time::Duration::from_secs_f64(1.0 / tick_rate);
    let render_delay = std::time::Duration::from_secs_f64(1.0 / frame_rate);
    let mut tick_interval = tokio::time::interval(tick_delay);
    let mut render_interval = tokio::time::interval(render_delay);

    loop {
        let stores = chat_page.stores();
        let mut dispatcher = Dispatcher::new(stores, &mut events);
        let dispatch = dispatcher.dispatch();

        let tick_delay = tick_interval.tick();
        let render_delay = render_interval.tick();
        tokio::select! {
            action = dispatch => {
                match action {
                    Action::Quit => break,
                    _ => continue,
                }
            },
            _ = tick_delay => {

            },
            _ = render_delay => {
                terminal.draw(|f| chat_page.render(f)).unwrap();
            }
        }
    }
}
