//! Mounting a component onto the terminal and keeping it there.

use std::io;

use crossterm::event::{self, Event};

use crate::component::Component;
use crate::screen::Screen;
use crate::tree::Tree;

/// Takes the terminal, mounts `C`, and draws it until a component calls the
/// closure from [`use_exit`](crate::use_exit).
///
/// The terminal is given back however this returns, including a panic.
pub fn run<C: Component>(props: C::Props) -> io::Result<()> {
    let mut screen = Screen::open()?;
    let mut tree = Tree::new::<C>(props);

    loop {
        screen.draw(|cells, area| tree.draw(cells, area))?;
        if tree.exiting() {
            return Ok(());
        }

        match event::read()? {
            Event::Key(key) => {
                if let Some(key) = press(&Event::Key(key)) {
                    tree.press(key);
                }
            }
            Event::Mouse(mouse) => {
                tree.mouse(mouse);
            }
            Event::Resize(..) => tree.redraw_all(),
            _ => {}
        }

        if tree.exiting() {
            return Ok(());
        }
    }
}

/// A crossterm key event as a key combination. A release is not a press.
fn press(event: &Event) -> Option<crokey::KeyCombination> {
    use crossterm::event::KeyEventKind;
    match event {
        Event::Key(key) if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) => {
            Some(crokey::KeyCombination::from(*key).normalized())
        }
        _ => None,
    }
}
