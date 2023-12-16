//! A Page is a composition of Views.
mod chat;

pub use chat::ChatPage;
/*

use std::{collections::HashMap, rc::Rc};

pub use chat::*;
use ratatui::{prelude::Rect, Frame};

use crate::dispatch::{PageRender, RenderContext, Store, ViewRender, ViewStore};
pub struct Page {
    parts: HashMap<(String, Rect), Option<Box<dyn ViewStore>>>,
}

impl PageRender for Page {
    fn render(&self, frame: &mut Frame) {
        for ((_, area), view) in self.parts.iter() {
            view.render(frame, RenderContext { area: *area });
        }
    }
}

impl Page {
    fn add<S: AsRef<str>>(&mut self, name: S, layout: Rc<[Rect]>) {
        let rect = vec![name.as_ref().to_string()]
            .into_iter()
            .cycle()
            .zip(layout.into_iter())
            .zip(vec![Default::default()].into_iter().cycle());
        self.parts.extend(rect)

        //.extend(layout.into_iter().chain(vec![name.as_ref().to_string()].into_iter().cycle()))
    }

    fn add_child(&self, parent: &str, child: impl ViewRender) {
        todo!()
    }

    fn stores(&mut self) -> &mut dyn Store {
        todo!()
    }
}

impl Default for Page {
    fn default() -> Self {
        Page { parts: Default::default() }
    }
}
*/
