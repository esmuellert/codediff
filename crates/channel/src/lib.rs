//! The contract every background worker follows, and the channel layer that
//! delivers results to the main loop without polling.

use std::sync::mpsc::{self, Receiver, Sender};

/// A typed sender that maps results into the app's event type.
///
/// The worker holds this and calls `send(result)`. It never imports the event
/// enum — the mapping is provided once at construction.
pub struct Emitter<T> {
    send: Box<dyn Fn(T) -> bool + Send>,
}

impl<T: Send + 'static> Emitter<T> {
    /// Creates an emitter that wraps `T` into `E` via `wrap`, then sends on `tx`.
    pub fn new<E: Send + 'static>(tx: Sender<E>, wrap: fn(T) -> E) -> Self {
        Self {
            send: Box::new(move |value| tx.send(wrap(value)).is_ok()),
        }
    }

    /// Creates an emitter backed by a local channel. For tests.
    pub fn local() -> (Self, Receiver<T>) {
        let (tx, rx) = mpsc::channel();
        let emitter = Self {
            send: Box::new(move |value| tx.send(value).is_ok()),
        };
        (emitter, rx)
    }

    /// Delivers one result. Returns `false` if the receiver is gone.
    pub fn send(&self, value: T) -> bool {
        (self.send)(value)
    }
}

/// A background worker that accepts requests and produces responses.
pub trait Worker {
    type Request;
    type Response;

    /// Sends a request. Never blocks. May be silently dropped if the worker
    /// is already busy — the caller re-asks later.
    fn send(&mut self, request: Self::Request);

    /// Whether anything is in flight.
    fn is_busy(&self) -> bool;

    /// Acknowledges a response arrived. Clears the in-flight state.
    fn received(&mut self, response: &Self::Response);
}
