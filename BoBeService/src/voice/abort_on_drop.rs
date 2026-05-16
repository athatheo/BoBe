//! Cascade `JoinHandle::abort()` on Drop. Vanilla JoinHandle drop leaves
//! the task running; for voice children (kokoro_task, filler_task) that
//! means 1-3s of dead-time holding `OfflineTts` Mutex after a parent
//! abort. Natural-completion paths call `into_inner()` to defuse.

use tokio::task::JoinHandle;

pub(crate) struct AbortOnDrop<T> {
    handle: Option<JoinHandle<T>>,
}

impl<T> AbortOnDrop<T> {
    pub(crate) fn new(handle: JoinHandle<T>) -> Self {
        Self {
            handle: Some(handle),
        }
    }

    /// Consume the wrapper without firing `abort()`. Use on the natural-
    /// completion path before `.await`-ing the handle yourself.
    #[allow(
        clippy::expect_used,
        reason = "by-value self ⇒ called at most once; second call would be a structural bug we want to surface, not silently no-op"
    )]
    pub(crate) fn into_inner(mut self) -> JoinHandle<T> {
        self.handle
            .take()
            .expect("AbortOnDrop::into_inner called twice")
    }
}

impl<T> Drop for AbortOnDrop<T> {
    fn drop(&mut self) {
        if let Some(h) = self.handle.take() {
            h.abort();
        }
    }
}
