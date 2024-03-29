mod cli;
mod dispatch;
mod events;
mod pages;
mod types;
mod util;
mod views;

use std::io::stderr;

use anyhow::{anyhow, Result};
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
    dispatch::{Action, Commands, Dispatcher, PageRender, Store, XMTP},
    events::Events,
    pages::ChatPage,
};

type CrosstermTerminal = Terminal<CrosstermBackend<std::io::Stderr>>;

#[tokio::main]
async fn main() -> Result<()> {
    // console_subscriber::init();
    self::util::init_logging().map_err(|_| anyhow!("Logging did not init"))?;
    #[allow(unused)]
    let app: cli::XChatApp = argh::from_env();

    let (actions, actions_subscription) = broadcast::channel::<Action>(100);

    let (xmtp_tx, xmtp_rx) = mpsc::channel(100);
    let (command_tx, command_rx) = mpsc::channel(100);

    // events
    let xmtp = XMTP::new(actions.clone(), xmtp_rx, app).spawn();
    let events = Events::new(actions.clone()).spawn();
    let commands = Commands::new(actions.clone(), xmtp_tx.clone(), command_rx).spawn();

    enable_raw_mode()?;
    stderr().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(stderr()))?;

    // views
    let chat_page = ChatPage::new(xmtp_tx, command_tx, actions.clone());

    if let Err(e) =
        render_loop(&mut terminal, actions_subscription, chat_page, 1_000.0, 120.0).await
    {
        log::error!("Error in render loop: {}", e);
        log::error!("Shutting down...")
    }

    events.abort();
    xmtp.abort();
    commands.abort();
    disable_raw_mode()?;
    stderr().execute(LeaveAlternateScreen)?;

    Ok(())
}

pub async fn render_loop(
    terminal: &mut CrosstermTerminal,
    mut events: Receiver<Action>,
    mut chat_page: ChatPage<'_>,
    tick_rate: f64,
    frame_rate: f64,
) -> Result<()> {
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
                    Action::Quit => break Ok(()),
                    _ => continue,
                }
            },
            _ = tick_delay => {
                continue
                // What do here?
            },
            _ = render_delay => {
                terminal.draw(|f| chat_page.render(f))?;
            }
        }
    }
}
