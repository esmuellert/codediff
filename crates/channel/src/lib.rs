//! The contract every background worker follows.
//!
//! A worker lives on its own thread, accepts requests, and produces responses.
//! The main thread sends work and collects results without blocking.

/// A background worker that accepts requests and produces responses.
pub trait Worker {
    type Request;
    type Response;

    /// Sends a request. Never blocks. May be silently dropped if the worker
    /// is already busy — the caller re-asks later.
    fn send(&mut self, request: Self::Request);

    /// Whether anything is in flight.
    fn is_busy(&self) -> bool;

    /// One response, if any has arrived. Never blocks.
    fn poll(&mut self) -> Option<Self::Response>;
}
