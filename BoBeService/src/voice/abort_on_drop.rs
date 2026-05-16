//! RAII wrapper that cascades `JoinHandle::abort()` when the parent task's
//! frame unwinds.
//!
//! Background: dropping a `tokio::task::JoinHandle` does NOT abort the
//! underlying task — the spawned future keeps running detached. For the
//! voice turn flow that means: when the parent `run_text_turn` task is
//! aborted via `TurnInFlight::join.abort()` (barge-in / disconnect /
//! cancel-phrase), its locally-spawned children (`kokoro_task`,
//! `filler_task`) survive the abort and continue holding shared resources
//! (`LocalKokoroTts::tts: Mutex<OfflineTts>`) for the rest of their
//! `spawn_blocking` synthesis call — 1-3 seconds of audible dead-time on
//! the next turn's first synth.
//!
//! Wrap the child `JoinHandle` in `AbortOnDrop` at spawn time. On natural
//! completion call `into_inner()` to consume the wrapper without aborting
//! and await/abort the handle yourself. If the parent task is aborted
//! before that point, the stack unwind runs this `Drop` and cancels the
//! child too.

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
