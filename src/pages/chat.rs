//! Layout handling of the terminal screen buffer

use std::{collections::HashMap, future::Future, ops::DerefMut, pin::Pin};

use ratatui::{prelude::*, Frame};
use tokio::sync::{broadcast::Sender as BroadcastSender, mpsc::Sender};

use crate::{
    dispatch::{Action, CommandAction, PageRender, Store, ViewStore, XMTPAction},
    views::{ChatArea, ChatRooms, InputBox},
};

#[derive(PartialEq, Eq, Copy, Clone, Hash)]
enum Child {
    ChatRooms,
    ChatArea,
    InputBox,
}

pub struct ChatPage {
    map: HashMap<Child, Box<dyn ViewStore>>,
}

impl ChatPage {
    /// Define the Layout for the Page
    pub fn new(
        xmtp: Sender<XMTPAction>,
        command: Sender<CommandAction>,
        events: BroadcastSender<Action>,
    ) -> Self {
        let mut map = HashMap::new();

        let (chat_area_view, input_box, chat_rooms) = (
            ChatArea::default(),
            InputBox::new(xmtp.clone(), command.clone()),
            ChatRooms::new(events),
        );

        map.insert(Child::ChatRooms, Box::new(chat_rooms) as Box<dyn ViewStore>);
        map.insert(Child::ChatArea, Box::new(chat_area_view) as Box<dyn ViewStore>);
        map.insert(Child::InputBox, Box::new(input_box) as Box<dyn ViewStore>);

        Self { map }
    }
}

mod buffers {
    pub const CHAT_AREA: usize = 1;
}

impl Store for ChatPage {
    fn update(&mut self, action: Action) -> Pin<Box<dyn Future<Output = ()> + '_>> {
        let future = async move {
            match action {
                Action::Resize(x, y) => log::debug!("Resizing Chat Page {x}:{y}"),
                _ => (),
            }
        };
        Box::pin(future)
    }

    fn stores(&mut self) -> Vec<&mut dyn Store> {
        self.map.values_mut().map(|s| s.deref_mut() as &mut dyn Store).collect()
    }
}

impl PageRender for ChatPage {
    fn render(&self, frame: &mut Frame) {
        let screen = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Percentage(10), Constraint::Percentage(90)])
            .split(frame.size());

        let chat_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![Constraint::Percentage(95), Constraint::Percentage(5)])
            .split(screen[buffers::CHAT_AREA]);

        self.map.get(&Child::ChatRooms).unwrap().render(frame, screen[0].into());
        self.map.get(&Child::ChatArea).unwrap().render(frame, chat_area[0].into());
        self.map.get(&Child::InputBox).unwrap().render(frame, chat_area[1].into());
    }
}
