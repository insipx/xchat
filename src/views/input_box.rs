use std::{future::Future, pin::Pin};

use anyhow::Result;
use crossterm::event::KeyEvent;
use ratatui::{
    widgets::{Block, Borders},
    Frame,
};
use tokio::sync::mpsc::Sender;
use tui_textarea::{CursorMove, Input, Key, TextArea};

use crate::{
    dispatch::{Action, CommandAction, RenderContext, Store, ViewRender, XMTPAction},
    types::Group,
};

#[derive(Debug, Clone)]
pub struct InputBox<'a> {
    text: String,
    xmtp: Sender<XMTPAction>,
    command: Sender<CommandAction>,
    focused_group: Group,
    text_area: TextArea<'a>,
}

impl<'a> InputBox<'a> {
    pub fn new(xmtp: Sender<XMTPAction>, command: Sender<CommandAction>) -> Self {
        let text_area = Self::text_area();
        Self { text: "".into(), xmtp, command, focused_group: Group::new_fake(0), text_area }
    }

    // TODO: Find a way NOT to recreate the textarea to preserve text history
    fn text_area() -> TextArea<'a> {
        let mut text_area = TextArea::from(Vec::<String>::new());
        text_area.set_block(Block::default().borders(Borders::ALL));
        text_area
    }

    async fn handle_enter(&mut self) -> Result<()> {
        if self.text_area.lines()[0].starts_with("/") {
            let text_area = std::mem::replace(&mut self.text_area, Self::text_area());
            let command = text_area.into_lines().remove(0);
            log::debug!("Got a command {}", &command);
            let cmd = command.strip_prefix("/").expect("Checked if start with `/`");
            let cmd = CommandAction::from_string(cmd.into(), &self.focused_group)?;
            self.command.send(cmd).await?;
            self.text.clear();
        } else {
            if self.text_area.is_empty() {
                return Ok(());
            }
            let text_area = std::mem::replace(&mut self.text_area, Self::text_area());
            self.text_area.move_cursor(CursorMove::Jump(0, 0));
            self.text_area.delete_line_by_head();
            let mut lines = text_area.into_lines();
            lines.join("\n");
            self.xmtp
                .send(
                    XMTPAction::SendMessage(self.focused_group.clone(), lines.drain(..).collect())
                        .into(),
                )
                .await
                .expect("Handle bad send");
        }

        Ok(())
    }

    // TODO:
    // Crossterm does not recognize Shift + Enter
    // which is annoying. Need another keybinding or figure out a workaround
    //
    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        match key.into() {
            Input { key: Key::Enter, ctrl: true, .. } => {
                self.text_area.move_cursor(CursorMove::End);
                self.text_area.insert_newline()
            }
            Input { key: Key::Enter, ctrl: false, .. } => {
                // we want to create a newline here.
                self.handle_enter().await?;
            }
            input => {
                self.text_area.input(input);
            }
        }
        Ok(())
    }

    pub fn lines(&'a self) -> &'a [String] {
        self.text_area.lines()
    }
}

impl<'a> Store for InputBox<'a> {
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

impl<'a> ViewRender for InputBox<'a> {
    fn render(&self, frame: &mut Frame, render_ctx: RenderContext) {
        let widget = self.text_area.widget();
        frame.render_widget(widget, render_ctx.area);
    }
}
