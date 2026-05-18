//! Screen-capture permission preflight via Apple's TCC database. Read-only,
//! no prompt — wizard separately invokes `CGRequestScreenCaptureAccess`
//! Swift-side when it wants to prompt. Module-local `#[allow(unsafe_code)]`
//! keeps the rest of `util/` under crate-level `deny(unsafe_code)`.

#![allow(unsafe_code)]

// SAFETY: declared in `<CoreGraphics/CGWindow.h>` as
// `bool CGPreflightScreenCaptureAccess(void)`. No-arg, no side effects.
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGPreflightScreenCaptureAccess() -> bool;
}

pub(crate) fn has_screen_capture_permission() -> bool {
    // SAFETY: see module-level comment.
    unsafe { CGPreflightScreenCaptureAccess() }
}
