//! Chat Rooms View
use std::{future::Future, pin::Pin};

use ratatui::{
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::dispatch::{Action, RenderContext, Store, ViewRender};

#[derive(Debug, Clone)]
pub struct ChatRooms {
    rooms: Vec<String>,
}

impl Store for ChatRooms {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        let future = async move {
            match action {
                Action::ChangeRoom(num) => {
                    log::debug!("Change Room to {}", num)
                }
                _ => (),
            }
        };
        Box::pin(future)
    }
}

impl ViewRender for ChatRooms {
    fn render(&self, frame: &mut Frame, render_ctx: RenderContext) {
        let rooms = Paragraph::new("Chat Rooms").block(Block::new().borders(Borders::ALL));
        frame.render_widget(rooms, render_ctx.area);
    }
}

impl Default for ChatRooms {
    fn default() -> Self {
        Self {
            rooms: vec![
                "#crypto".into(),
                "#philosophy".into(),
                "#anime".into(),
                "#vidyagames".into(),
            ],
        }
    }
}
