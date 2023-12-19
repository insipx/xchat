//! Chat Rooms View
use std::{future::Future, pin::Pin};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{prelude::*, widgets::*, Frame};
use tokio::sync::broadcast::Sender;

use crate::{
    dispatch::{Action, RenderContext, Store, ViewRender},
    types::{Group, GroupIdWrapper},
};

#[derive(Debug, Clone)]
pub struct ChatRooms {
    rooms: Vec<String>,
    groups: Vec<Group>,
    /// index into groups vector
    focused: usize,
    events: Sender<Action>,
}

// TODO: Search `.unwrap`, `.expect`, `let _ =`

impl ChatRooms {
    pub fn new(events: Sender<Action>) -> Self {
        Self { rooms: vec!["xchat".into()], groups: vec![Group::new_fake(0)], focused: 0, events }
    }

    async fn handle_key_event(&mut self, key: KeyEvent) -> Result<()> {
        if key.modifiers.intersects(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('n') => {
                    self.focus_next();
                    self.events.send(Action::SetFocusedGroup(self.groups[self.focused].clone()))?;
                }
                KeyCode::Char('p') => {
                    self.focus_previous();
                    self.events.send(Action::SetFocusedGroup(self.groups[self.focused].clone()))?;
                }
                _ => (),
            };
        }
        Ok(())
    }

    fn focus_next(&mut self) {
        if self.focused < (self.groups.len() - 1) {
            self.focused += 1;
        }
    }

    fn focus_previous(&mut self) {
        self.focused = self.focused.saturating_sub(1);
    }
}

impl Store for ChatRooms {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = Result<()>> + '_>> {
        let future = async move {
            match action {
                Action::KeyPress(key) => self.handle_key_event(key).await?,
                Action::NewGroups(groups) => {
                    log::debug!("Got new groups {:?}", groups);
                    let groups = groups.into_iter();
                    self.groups.extend(groups.clone());
                    for group in groups {
                        self.rooms.push(format!("{}", &GroupIdWrapper::from(group.id)));
                    }
                }
                _ => (),
            };
            Ok(())
        };
        Box::pin(future)
    }
}

impl ViewRender for ChatRooms {
    fn render(&self, frame: &mut Frame, render_ctx: RenderContext) {
        let mut rooms = self.rooms.iter().cloned().map(ListItem::new).collect::<Vec<_>>();
        rooms[self.focused] = rooms[self.focused].clone().style(Style::new().fg(Color::LightGreen));

        frame.render_widget(
            List::new(rooms).block(Block::new().borders(Borders::ALL)),
            render_ctx.area,
        );
    }
}
