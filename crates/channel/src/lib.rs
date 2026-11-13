//! Delivers results from a background thread to the main loop without polling.

use std::sync::mpsc::Sender;

/// A typed sender that maps results into the app's event type.
///
/// The worker holds this and calls `send(result)`. It never imports the event
/// enum — the mapping is provided once at construction.
pub struct Emitter<T> {
    send: Box<dyn Fn(T) -> bool + Send>,
}

impl<T: Send + 'static> Emitter<T> {
    /// Creates an emitter that wraps `T` into `E` via `wrap`, then sends on `tx`.
    pub fn new<E, F>(tx: Sender<E>, wrap: F) -> Self
    where
        E: Send + 'static,
        F: Fn(T) -> E + Send + 'static,
    {
        Self {
            send: Box::new(move |value| tx.send(wrap(value)).is_ok()),
        }
    }

    /// Delivers one result. Returns `false` if the receiver is gone.
    pub fn send(&self, value: T) -> bool {
        (self.send)(value)
    }
}
