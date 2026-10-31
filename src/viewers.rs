//! Counts connected dashboard viewers so the sampler can sleep when nobody is watching.

use std::sync::{Arc, Condvar, Mutex};

#[derive(Default)]
pub struct Viewers {
    count: Mutex<usize>,
    changed: Condvar,
}

/// Held for the lifetime of one SSE connection. Dropping it decrements the count.
pub struct ViewerGuard(Arc<Viewers>);

impl Viewers {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn enter(self: &Arc<Self>) -> ViewerGuard {
        *self.count.lock().unwrap() += 1;
        self.changed.notify_all();
        ViewerGuard(Arc::clone(self))
    }

    pub fn count(&self) -> usize {
        *self.count.lock().unwrap()
    }

    /// Block the calling thread until at least one viewer is connected.
    pub fn wait_for_viewer(&self) {
        let mut n = self.count.lock().unwrap();
        while *n == 0 {
            n = self.changed.wait(n).unwrap();
        }
    }
}

impl Drop for ViewerGuard {
    fn drop(&mut self) {
        let mut n = self.0.count.lock().unwrap();
        *n = n.saturating_sub(1);
        self.0.changed.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn counts_guards() {
        let v = Viewers::new();
        assert_eq!(v.count(), 0);
        let a = v.enter();
        let b = v.enter();
        assert_eq!(v.count(), 2);
        drop(a);
        assert_eq!(v.count(), 1);
        drop(b);
        assert_eq!(v.count(), 0);
    }

    #[test]
    fn wait_blocks_until_a_viewer_arrives() {
        let v = Viewers::new();
        let v2 = Arc::clone(&v);
        let h = std::thread::spawn(move || v2.wait_for_viewer());
        std::thread::sleep(Duration::from_millis(100));
        assert!(!h.is_finished(), "must block while nobody is watching");
        let _g = v.enter();
        h.join().unwrap();
    }

    #[test]
    fn wait_returns_immediately_when_already_watched() {
        let v = Viewers::new();
        let _g = v.enter();
        v.wait_for_viewer();
    }
}
