//! RAII guard that clears a shared `Arc<AtomicBool>` on drop. Used as a
//! single-flight gate for user messages, capture cycles, and voice turns —
//! all three were near-identical drop impls before this util landed.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub(crate) struct AtomicFlagGuard {
    flag: Arc<AtomicBool>,
}

impl AtomicFlagGuard {
    /// Construct without CAS. The caller is responsible for having set the
    /// flag to `true`; the guard only handles the release side.
    pub(crate) fn new(flag: Arc<AtomicBool>) -> Self {
        Self { flag }
    }

    /// CAS-acquire: returns `Some(guard)` when the flag transitioned
    /// `false → true`; `None` if another task already holds it.
    pub(crate) fn try_acquire(flag: Arc<AtomicBool>) -> Option<Self> {
        if flag
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return None;
        }
        Some(Self { flag })
    }
}

impl Drop for AtomicFlagGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;

    #[test]
    fn drop_clears_the_flag() {
        let flag = Arc::new(AtomicBool::new(true));
        let guard = AtomicFlagGuard::new(Arc::clone(&flag));
        assert!(flag.load(Ordering::Acquire));
        drop(guard);
        assert!(!flag.load(Ordering::Acquire));
    }

    #[test]
    fn drop_clears_on_panic() {
        let flag = Arc::new(AtomicBool::new(true));
        let flag_clone = Arc::clone(&flag);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
            let _guard = AtomicFlagGuard::new(flag_clone);
            panic!("simulated failure");
        }));
        assert!(result.is_err());
        assert!(!flag.load(Ordering::Acquire));
    }

    #[test]
    fn try_acquire_fails_when_already_set() {
        let flag = Arc::new(AtomicBool::new(false));
        let g1 = AtomicFlagGuard::try_acquire(Arc::clone(&flag));
        assert!(g1.is_some());
        let g2 = AtomicFlagGuard::try_acquire(Arc::clone(&flag));
        assert!(g2.is_none());
    }
}
