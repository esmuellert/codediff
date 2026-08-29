//! Mounting a component onto the terminal and keeping it there.

use std::io;
use std::sync::mpsc::{Receiver, Sender};
use std::thread;

use crossterm::event::{self, Event};

use crate::screen::Screen;
use crate::tree::Tree;

/// What the loop does next, after the application has answered an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Draw if the tree wants it, then wait for the next event.
    Continue,
    /// Give the terminal back, stop, and take it again on resume.
    Suspend,
    /// Leave.
    Quit,
}

/// Takes the terminal, mounts `tree`, and draws it until something stops it.
///
/// Every source of events feeds one channel: the input thread this spawns,
/// and whatever the application gave a `Sender` to. `respond` is called with
/// each one and says what to do next.
///
/// The terminal is given back however this returns, including a panic.
pub fn run<E: Send + 'static>(
    tree: &mut Tree,
    events: Receiver<E>,
    to_events: Sender<E>,
    wrap_input: fn(Event) -> E,
    mut respond: impl FnMut(E, &mut Tree) -> Flow,
) -> io::Result<()> {
    let mut screen = Screen::open()?;

    // After raw mode, or the first keys are echoed to the shell.
    spawn_input(to_events, wrap_input);

    loop {
        screen.draw(|cells, area| tree.draw(cells, area))?;
        // A layout effect can write state. Drawing again settles it.
        // A bounded retry prevents an infinite loop from a bug.
        let mut extra = 0;
        while tree.needs_draw() && !tree.exiting() && extra < 4 {
            screen.draw(|cells, area| tree.draw(cells, area))?;
            extra += 1;
        }
        if tree.exiting() {
            return Ok(());
        }

        let Ok(event) = events.recv() else {
            return Ok(());
        };

        match respond(event, tree) {
            Flow::Quit => return Ok(()),
            Flow::Suspend => {
                #[cfg(unix)]
                screen.suspend()?;
                tree.redraw_all();
            }
            Flow::Continue => {}
        }
        if tree.exiting() {
            return Ok(());
        }
    }
}

/// Routes one terminal event into the tree. What an application calls for
/// its own `Terminal` variant.
pub fn deliver_input(tree: &mut Tree, event: &Event) {
    match event {
        Event::Key(_) => {
            if let Some(key) = press(event) {
                tree.press(key);
            }
        }
        Event::Mouse(mouse) => {
            tree.mouse(*mouse);
        }
        Event::Resize(..) => tree.redraw_all(),
        _ => {}
    }
}

/// The terminal reader. Blocked in `event::read()` until a key arrives.
fn spawn_input<E: Send + 'static>(to_events: Sender<E>, wrap: fn(Event) -> E) {
    thread::Builder::new()
        .name("input".to_owned())
        .spawn(move || {
            while let Ok(event) = event::read() {
                if to_events.send(wrap(event)).is_err() {
                    break;
                }
            }
        })
        .expect("the input thread starts");
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
