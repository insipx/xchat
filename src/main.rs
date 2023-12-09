use std::{
    // hide_line
    io::{stderr, Result}, // hide_line
    thread::sleep,        // hide_line
    time::Duration,       // hide_line
}; // hide_line

// hide_line
use crossterm::{
    // hide_line
    terminal::{EnterAlternateScreen, LeaveAlternateScreen}, // hide_line
    ExecutableCommand,                                      // hide_line
}; // hide_line
use ratatui::{prelude::*, widgets::*}; // hide_line
                                       // hide_line
fn main() -> Result<()> {
    // hide_line
    let should_enter_alternate_screen = std::env::args().nth(1).unwrap().parse::<bool>().unwrap(); // hide_line
    if should_enter_alternate_screen {
        // hide_line
        stderr().execute(EnterAlternateScreen)?; // remove this line
    } // hide_line

    let mut terminal = Terminal::new(CrosstermBackend::new(stderr()))?;

    terminal.draw(|f| {
        f.render_widget(Paragraph::new("Hello World!"), Rect::new(10, 20, 20, 1));
    })?;
    sleep(Duration::from_secs(2));

    if should_enter_alternate_screen {
        // hide_line
        stderr().execute(LeaveAlternateScreen)?; // remove this line
    } // hide_line
    Ok(()) // hide_line
} // hide_line
