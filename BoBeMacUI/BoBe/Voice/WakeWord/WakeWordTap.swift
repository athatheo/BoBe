import Foundation

/// A continuously-listening wake-word detector. Runs while the mic
/// permission is granted, voice is enabled, and the user has opted in.
/// On detection, fires a `@MainActor` callback that opens the voice WS
/// — the same path manual mic-button taps drive.
///
/// Implementations:
///   - `NoOpWakeWordTap` — default; logs once and does nothing. Picks up
///     when the user toggles the wake-word setting on but the real engine
///     hasn't been wired yet.
///   - `LiveKitWakeWordTap` (forthcoming) — wraps LiveKit's `WakeWordModel`
///     + `WakeWordListener` once the SPM dep lands. See the comment block
///     below for the integration plan.
///
/// Concurrency: Sendable so the implementation can be an actor or a
/// `@MainActor` class. Callbacks bridge to the main actor at the boundary.
protocol WakeWordTap: Sendable {
    /// Begin listening. Idempotent — calling `start` on an already-running
    /// tap is a no-op. Throws if the model fails to load.
    func start() async throws

    /// Stop listening. Idempotent. Safe to call even when never started.
    func stop() async

    /// Set the detection callback. Replaces any previously installed
    /// handler. Called on `@MainActor` so VoicePipeline updates are
    /// straightforward.
    func onDetected(_ handler: @escaping @Sendable @MainActor (WakeWordDetection) -> Void) async
}

/// One detection event from the underlying engine. `name` is whatever the
/// trained classifier reports (the phrase, lower-cased); `confidence` is
/// the post-threshold probability in [0..1].
struct WakeWordDetection: Sendable {
    let name: String
    let confidence: Float
    let timestamp: Date
}

/// Default tap when no wake-word engine is configured. Logs once on first
/// `start()` and otherwise stays quiet. Lets the rest of the UI render the
/// "Wake word" setting row without depending on the actual ML stack.
actor NoOpWakeWordTap: WakeWordTap {
    private var loggedOnce = false

    func start() async throws {
        if self.loggedOnce { return }
        self.loggedOnce = true
        // Log via stdout — Logger isn't worth pulling in for a single
        // path that disappears once the real tap lands.
        print("[WakeWordTap] No engine wired; tap is a no-op.")
    }

    func stop() async {}

    func onDetected(_ handler: @escaping @Sendable @MainActor (WakeWordDetection) -> Void) async {
        // Discard. The real tap stores + fires this.
        _ = handler
    }
}

// MARK: - Integration plan (livekit-wakeword)
//
// `livekit/livekit-wakeword` Apache-2.0 — the recommended engine per the
// May 2026 audit (Reddit/HA-forum review). Conv-attention head with a
// custom-trainable `.onnx`. Runs on CoreML / ANE via the Microsoft
// `onnxruntime-swift-package-manager` dep.
//
// Layout snag: the Swift package lives at `swift/Package.swift` inside
// the Python-primary repo, NOT at the repo root. SPM doesn't natively
// fetch subdirectory packages from a remote URL. Two viable resolutions
// before the real tap can be wired:
//
//   A. Vendor the 3 public + 7 internal Swift files (~50 KB) plus the
//      2 .onnx resources (~2.4 MB) into `BoBe/Voice/WakeWord/Vendor/`
//      with an Apache-2.0 NOTICE. Then add the `onnxruntime-swift-
//      package-manager` dep to `BoBeMacUI/Package.swift`. Manual sync
//      against upstream on every refresh.
//   B. Git submodule the whole `livekit/livekit-wakeword` repo at
//      `Vendor/livekit-wakeword/` (drags Python source we don't need),
//      add `.package(path: "../Vendor/livekit-wakeword/swift")`. Easier
//      upstream sync but submodules add a clone-time setup step.
//
// Once (A) or (B) lands, this file gets a `LiveKitWakeWordTap` actor
// that wraps `WakeWordModel(models: [classifierURL], sampleRate: 16_000)`
// + `WakeWordListener`. Then the `VoicePipeline` reads its `detections()`
// async-sequence and fires `connect(daemonBaseURL:)` per detection. See
// `voice-architecture.md` for the planned state-machine hook.
