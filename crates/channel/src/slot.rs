//! Latest-value slot shared by a caller and a worker.

use std::sync::{Arc, Condvar, Mutex};

/// Shared state between the two halves of a [`Slot`].
struct Inner<T> {
    value: Mutex<Option<T>>,
    ready: Condvar,
    busy: Mutex<bool>,
}

/// A slot where a newer request replaces a waiting request.
///
/// The worker loop blocks until a value arrives and then runs the job.
pub struct Slot<T> {
    inner: Arc<Inner<T>>,
}

impl<T: Send + 'static> Slot<T> {
    /// Creates a slot and a worker loop for `job`.
    ///
    /// The loop stops when `job` returns `false` or the slot is dropped.
    pub fn new<F>(mut job: F) -> (Self, impl FnOnce() + Send + 'static)
    where
        F: FnMut(T) -> bool + Send + 'static,
    {
        let inner = Arc::new(Inner {
            value: Mutex::new(None),
            ready: Condvar::new(),
            busy: Mutex::new(false),
        });
        let worker = Arc::clone(&inner);
        let worker_loop = move || {
            loop {
                let item = {
                    let mut slot = worker.value.lock().unwrap();
                    while slot.is_none() {
                        if Arc::strong_count(&worker) == 1 {
                            *worker.busy.lock().unwrap() = false;
                            return;
                        }
                        *worker.busy.lock().unwrap() = false;
                        slot = worker.ready.wait(slot).unwrap();
                    }
                    *worker.busy.lock().unwrap() = true;
                    slot.take().unwrap()
                };
                if !job(item) {
                    *worker.busy.lock().unwrap() = false;
                    return;
                }
            }
        };
        (Self { inner }, worker_loop)
    }

    /// Places a value in the slot, replacing whatever was waiting.
    pub fn put(&self, value: T) {
        *self.inner.value.lock().unwrap() = Some(value);
        *self.inner.busy.lock().unwrap() = true;
        self.inner.ready.notify_one();
    }

    /// Whether a value is waiting or the worker is processing one.
    pub fn is_busy(&self) -> bool {
        *self.inner.busy.lock().unwrap()
    }
}
