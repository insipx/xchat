//! Dispatcher for our stores
mod commands;
mod xmtp;

use std::{
    collections::HashMap,
    future::{self, Future},
    pin::Pin,
};

use anyhow::Result;
pub use commands::*;
use crossterm::event::KeyEvent;
use futures::future::join_all;
use ratatui::{prelude::Rect, Frame};
use tokio::sync::broadcast::Receiver;
pub use xmtp::*;
use xmtp_mls::storage::group_message::StoredGroupMessage;

use crate::types::{Group, GroupId};

/// Generic Dispatcher that dispatches actions
pub struct Dispatcher<'a> {
    stores: Vec<&'a mut dyn Store>,
    events: &'a mut Receiver<Action>,
}

impl<'a> Dispatcher<'a> {
    pub fn new(stores: Vec<&'a mut dyn Store>, events: &'a mut Receiver<Action>) -> Self {
        Self { stores, events }
    }

    /// Asyncronously broadcasts an action to all `Stores`. Logs any errors resulting from them.
    pub async fn dispatch(&mut self) -> Action {
        let action = self.events.recv().await.unwrap();

        match action {
            Action::Quit => return Action::Quit,
            _ => (),
        };

        let mut dispatches = vec![];

        for store in self.stores.iter_mut() {
            dispatches.push(store.update(action.clone()));
        }

        let results = join_all(dispatches).await;
        for result in results {
            match result {
                Err(e) => log::error!("{}", e),
                _ => (),
            }
        }
        Action::Noop
    }
}

/// Trait that stores some state that is updated based on the defined [`Action`]
pub trait Store {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = Result<()>> + '_>>;
    fn stores(&mut self) -> Vec<&mut dyn Store> {
        Vec::new()
    }
}

impl Store for () {
    fn update(&mut self, _: Action) -> Pin<Box<dyn Future<Output = Result<()>> + '_>> {
        Box::pin(future::ready(Ok(())))
    }
}

/// Specify how to render the view to the Terminal [`Frame`].
/// `Context` is an structure with state needed for the rendering of a view.
///
/// # Example
/// // Todo
pub trait ViewRender {
    fn render(&self, frame: &mut Frame, render_ctx: RenderContext);
}

impl ViewRender for () {
    fn render(&self, _: &mut Frame, _: RenderContext) {}
}

pub trait ViewStore: ViewRender + Store {}

impl<T> ViewStore for T where T: ViewRender + Store {}

pub trait PageRender {
    fn render(&self, frame: &mut Frame);
}

/// Everything that can happen in the applications
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Quit,
    Resume,
    Suspend,
    Tick,
    RenderTick,
    KeyPress(KeyEvent),
    Resize(u16, u16),
    EnterNormal,
    EnterInsert,
    FakeMessage(GroupId, (String, String)),
    ReceiveMessages(HashMap<GroupId, Vec<StoredGroupMessage>>),
    SetFocusedGroup(Group),
    NewGroups(Vec<Group>),
    ChangeRoom(usize),
    XMTP(XMTPAction),
    Command(CommandAction),
    Noop,
}

pub struct RenderContext {
    pub area: Rect,
}

impl From<Rect> for RenderContext {
    fn from(rect: Rect) -> RenderContext {
        RenderContext { area: rect }
    }
}
