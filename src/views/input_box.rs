use std::{future::Future, pin::Pin};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use tokio::sync::mpsc::Sender;

use crate::{
    dispatch::{Action, CommandAction, RenderContext, Store, ViewRender, XMTPAction},
    types::{Coords, Group},
};

#[derive(Debug, Clone)]
pub struct InputBox {
    text: String,
    cursor_position: Coords,
    xmtp: Sender<XMTPAction>,
    command: Sender<CommandAction>,
    focused_group: Group,
}

impl InputBox {
    pub fn new(xmtp: Sender<XMTPAction>, command: Sender<CommandAction>) -> Self {
        Self {
            text: "".into(),
            cursor_position: Default::default(),
            xmtp,
            command,
            focused_group: Group::new_fake(0),
        }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if !key.modifiers.is_empty() {
            return Ok(());
        }

        match key.code {
            KeyCode::Char(c) => {
                self.text.push(c);
                self.cursor_position.x += 1;
            }
            KeyCode::Backspace => {
                let _ = self.text.pop();
                self.cursor_position.x = self.cursor_position.x.saturating_sub(1);
            }
            KeyCode::Enter => {
                if self.text.starts_with("/") {
                    log::debug!("Got a command {}", &self.text);
                    let cmd = self.text.drain(1..).collect::<String>();
                    let cmd = CommandAction::from_string(cmd, &self.focused_group)?;
                    self.command.send(cmd).await.expect("Handle bad send");
                    self.text.clear();
                } else {
                    self.xmtp
                        .send(
                            XMTPAction::SendMessage(
                                self.focused_group.clone(),
                                self.text.drain(..).collect(),
                            )
                            .into(),
                        )
                        .await
                        .expect("Handle bad send");
                }
            }
            _ => (),
        };
        Ok(())
    }
}

impl Store for InputBox {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = Result<()>> + '_>> {
        let future = async move {
            match action {
                Action::KeyPress(key) => self.handle_key_event(key).await?,
                Action::SetFocusedGroup(group) => self.focused_group = group,
                _ => (),
            };
            Ok(())
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
