import AppKit
import Foundation
import Observation
import os

/// `@Observable` driver for the daemon's `/auth/copilot/login` device-flow
/// endpoints. Maps the wire enum onto a UI-facing state machine and owns
/// the SSE-consuming Task. SwiftUI views read `runner.state` and call
/// `start()` / `cancel()` / `openVerificationURL()` / `copyCode()`.
///
/// Auth ownership note: the **bundled `copilot` CLI** drives the device
/// flow against GitHub's own approved OAuth client; BoBe never sees the
/// token. Treat this class as a thin remote-control surface for that CLI.
@MainActor
@Observable
final class CopilotLoginRunner {
    enum State: Equatable {
        case idle
        case preparing
        case awaitingUser(url: URL, code: String)
        case polling(url: URL, code: String)
        case completed
        case failed(message: String)
        case canceled
    }

    private(set) var state: State = .idle
    private(set) var didCopyCode = false

    private var streamTask: Task<Void, Never>?
    private let logger = Logger(subsystem: "com.bobe.app", category: "CopilotLoginRunner")

    func start() {
        // Avoid double-start: if we already have an in-flight stream, just
        // re-attach (subscribing late returns the watch channel's current
        // value, so the UI never lands on a blank sheet).
        if self.streamTask != nil { return }

        self.state = .preparing
        self.didCopyCode = false
        let client = DaemonClient.shared

        self.streamTask = Task { [weak self] in
            guard let self else { return }
            do {
                try await client.startCopilotLogin()
            } catch {
                self.logger.warning("startCopilotLogin failed: \(error.localizedDescription, privacy: .public)")
                self.state = .failed(message: error.localizedDescription)
                self.streamTask = nil
                return
            }

            do {
                try await client.streamCopilotLogin { [weak self] phase in
                    Task { @MainActor [weak self] in
                        self?.apply(phase: phase)
                    }
                }
            } catch {
                self.logger.warning("streamCopilotLogin error: \(error.localizedDescription, privacy: .public)")
                if case .failed = self.state {} else if case .completed = self.state {} else if case .canceled = self.state {} else {
                    self.state = .failed(message: error.localizedDescription)
                }
            }
            self.streamTask = nil
        }
    }

    /// Asks the daemon to kill the in-flight CLI. The watch channel will
    /// land on `.canceled` shortly after; we also set local state eagerly
    /// so the sheet's Cancel button doesn't feel laggy.
    func cancel() {
        self.streamTask?.cancel()
        self.streamTask = nil
        // Don't bash the local state if we already saw a terminal phase.
        switch self.state {
        case .completed, .failed, .canceled: break
        default: self.state = .canceled
        }
        Task {
            try? await DaemonClient.shared.cancelCopilotLogin()
        }
    }

    /// Resets to `.idle` so the sheet can present a fresh sign-in attempt
    /// after a Failed or Canceled terminal phase.
    func reset() {
        self.state = .idle
        self.didCopyCode = false
    }

    func openVerificationURL() {
        guard case let .awaitingUser(url, _) = self.state else {
            if case let .polling(url, _) = self.state {
                NSWorkspace.shared.open(url)
            }
            return
        }
        NSWorkspace.shared.open(url)
    }

    func copyCode() {
        let code: String
        switch self.state {
        case let .awaitingUser(_, c), let .polling(_, c):
            code = c
        default:
            return
        }
        let pasteboard = NSPasteboard.general
        pasteboard.clearContents()
        pasteboard.setString(code, forType: .string)
        self.didCopyCode = true
        // Auto-clear the "Copied" affordance after 2s so a second copy can
        // re-show the affirmation.
        Task { @MainActor [weak self] in
            try? await Task.sleep(for: .seconds(2))
            self?.didCopyCode = false
        }
    }

    private func apply(phase: CopilotLoginPhase) {
        switch phase {
        case .preparing:
            self.state = .preparing
        case let .awaitingUser(url, code):
            guard let parsed = URL(string: url) else {
                self.state = .failed(message: "Invalid verification URL: \(url)")
                return
            }
            self.state = .awaitingUser(url: parsed, code: code)
        case let .polling(url, code):
            guard let parsed = URL(string: url) else {
                self.state = .failed(message: "Invalid verification URL: \(url)")
                return
            }
            self.state = .polling(url: parsed, code: code)
        case .completed:
            self.state = .completed
        case let .failed(message):
            self.state = .failed(message: message)
        case .canceled:
            self.state = .canceled
        }
    }
}
