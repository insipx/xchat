use std::{future::Future, pin::Pin};

use crossterm::event::KeyCode;
use ratatui::{
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tokio::sync::mpsc::Sender;

use crate::{
    dispatch::{Action, RenderContext, Store, ViewRender, XMTPAction},
    types::Coords,
};

#[derive(Debug, Clone)]
pub struct InputBox {
    text: String,
    cursor_position: Coords,
    tx: Sender<XMTPAction>,
}

impl InputBox {
    pub fn new(tx: Sender<XMTPAction>) -> Self {
        Self { text: "".into(), cursor_position: Default::default(), tx }
    }

    async fn handle_key_event(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char(c) => {
                self.text.push(c);
                self.cursor_position.x += 1;
            }
            KeyCode::Backspace => {
                let _ = self.text.pop();
                self.cursor_position.x = self.cursor_position.x.saturating_sub(1);
            }
            KeyCode::Enter => {
                // TODO: Consider spawning this.
                self.tx
                    .send(XMTPAction::SendMessage(self.text.drain(..).collect()).into())
                    .await
                    .expect("Handle bad send");
            }
            _ => (),
        }
    }
}

impl Store for InputBox {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        let future = async move {
            match action {
                Action::KeyPress(code) => self.handle_key_event(code).await,
                _ => (),
            }
        };

        Box::pin(future)
    }
}

impl ViewRender for InputBox {
    fn render(&self, frame: &mut Frame, render_ctx: RenderContext) {
        let input =
            Paragraph::new(self.text.as_str()).block(Block::default().borders(Borders::ALL));
        frame.render_widget(input, render_ctx.area);
    }
}
