use crate::util::atomic_flag_guard::AtomicFlagGuard;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
#[derive(Debug, Clone, Copy)]
pub(crate) enum TurnSource {
    User,
    Capture,
    Goal,
    Checkin,
    Consolidation,
    Maintenance,
}
pub(crate) struct TurnAdmission {
    occupied: Arc<AtomicBool>,
}
impl TurnAdmission {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            occupied: Arc::new(AtomicBool::new(false)),
        })
    }
    pub(crate) fn try_admit(&self, _source: TurnSource) -> Option<AtomicFlagGuard> {
        AtomicFlagGuard::try_acquire(Arc::clone(&self.occupied))
    }
    pub(crate) fn is_idle(&self) -> bool {
        !self.occupied.load(std::sync::atomic::Ordering::Acquire)
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "tests panic on precondition failures")]
mod tests {
    use super::*;
    #[test]
    fn sources_share_permit() {
        let a = TurnAdmission::new();
        let p = a.try_admit(TurnSource::Goal).unwrap();
        assert!(a.try_admit(TurnSource::Consolidation).is_none());
        assert!(a.try_admit(TurnSource::Maintenance).is_none());
        assert!(a.try_admit(TurnSource::User).is_none());
        drop(p);
        assert!(a.try_admit(TurnSource::Capture).is_some());
    }
}
