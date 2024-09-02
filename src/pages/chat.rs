//! Layout handling of the terminal screen buffer

use std::{future::Future, pin::Pin};

use anyhow::Result;
use ratatui::{prelude::*, Frame};
use tokio::sync::{broadcast::Sender as BroadcastSender, mpsc::Sender};

use crate::{
    dispatch::{Action, CommandAction, PageRender, Store, ViewRender, XMTPAction},
    views::{ChatArea, ChatRooms, InputBox},
};

const MIN_CHAT_HEIGHT: usize = 1;

pub struct ChatPage<'a> {
    input_box: InputBox<'a>,
    chat_area: ChatArea,
    rooms: ChatRooms,
}

impl ChatPage<'_> {
    /// Define the Layout for the Page
    pub fn new(
        xmtp: Sender<XMTPAction>,
        command: Sender<CommandAction>,
        events: BroadcastSender<Action>,
    ) -> Self {
        let (input_box, chat_area, rooms) = (
            InputBox::new(xmtp.clone(), command.clone()),
            ChatArea::default(),
            ChatRooms::new(events),
        );

        Self { input_box, chat_area, rooms }
    }
}

mod buffers {
    pub const CHAT_AREA: usize = 1;
}

impl Store for ChatPage<'_> {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = Result<()>> + '_>> {
        let future = async move {
            match action {
                Action::Resize(x, y) => log::debug!("Resizing Chat Page {x}:{y}"),
                _ => (),
            }
            Ok(())
        };
        Box::pin(future)
    }

    fn stores(&mut self) -> Vec<&mut dyn Store> {
        vec![
            &mut self.input_box as &mut dyn Store,
            &mut self.chat_area as &mut dyn Store,
            &mut self.rooms as &mut dyn Store,
        ]
    }
}

impl PageRender for ChatPage<'_> {
    fn render(&self, frame: &mut Frame) {
        let screen = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(10), Constraint::Percentage(90)])
            .split(frame.area());

        let height = std::cmp::max(self.input_box.lines().len(), MIN_CHAT_HEIGHT) as u16 + 2;

        let chat_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Min(10), Constraint::Length(height)])
            .split(screen[buffers::CHAT_AREA]);

        self.rooms.render(frame, screen[0].into());
        self.chat_area.render(frame, chat_area[0].into());
        self.input_box.render(frame, chat_area[1].into());
    }
}
