//! The Chat Area View
mod types;

use std::{future::Future, pin::Pin};

use ratatui::{prelude::*, widgets::*, Frame};

use self::types::*;
use crate::dispatch::{Action, RenderContext, Store, ViewRender};

#[derive(Clone)]
pub struct ChatArea {
    messages: Messages,
}

impl Default for ChatArea {
    fn default() -> Self {
        let mut messages = Messages::default();
        messages.add(&vec![0], Message { user: "xchat".into(), text: WELCOME_MESSAGE.into() });
        messages.add(
            &vec![0],
            Message { user: "xchat".into(), text: "Hello! Welcome to xChat".into() },
        );
        messages.focused = vec![0];

        Self { messages }
    }
}
// TODO: set focused group

impl Store for ChatArea {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        let future = async {
            match action {
                Action::ReceiveMessage(group_id, (user, text)) => {
                    self.messages.add(&group_id, Message { text, user });
                }
                Action::ReceiveMessages(messages) => self.messages.add_group_messages(messages),
                Action::SetFocusedGroup(group) => self.messages.set_focus(&group.id),
                _ => (),
            }
        };
        Box::pin(future)
    }
}

impl ViewRender for ChatArea {
    fn render(&self, frame: &mut Frame, render_ctx: RenderContext) {
        let (users, messages) = self.messages.get();
        let user_style = Style::new().fg(Color::LightCyan);
        let users =
            users.into_iter().map(ListItem::new).map(|i| i.style(user_style)).collect::<Vec<_>>();
        let messages = messages.into_iter().map(ListItem::new).collect::<Vec<_>>();

        let chat_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(7), Constraint::Percentage(93)])
            .split(render_ctx.area);

        let messages = List::new(messages)
            .block(Block::new().borders(Borders::TOP | Borders::BOTTOM | Borders::RIGHT));
        let users = List::new(users)
            .block(Block::new().borders(Borders::LEFT | Borders::TOP | Borders::BOTTOM));

        frame.render_widget(users, chat_area[0]);
        frame.render_widget(messages, chat_area[1]);
    }
}