import SwiftUI

/// Native sheet that drives the daemon's `/auth/copilot/login` device flow.
/// Replaces the previous osascript→Terminal handoff so the user never
/// leaves BoBe. The installed CLI still owns the OAuth flow; this view is
/// just a remote control on top of it.
///
/// Lifecycle: callers present the sheet with `.sheet(isPresented: …)`, the
/// runner starts on `.task`, and the sheet dismisses itself on
/// `.completed` (firing `onSuccess`). Terminal failure / cancel keep the
/// sheet open so the user can retry without re-presenting.
struct CopilotSignInSheet: View {
    /// Called when the runner reaches `.completed`. The presenter is
    /// responsible for refreshing `/auth/status` and dismissing.
    let onSuccess: () -> Void
    /// Called when the user clicks Cancel or Done after a terminal phase.
    /// Presenter dismisses.
    let onClose: () -> Void

    @Environment(\.theme) private var theme
    @State private var runner = CopilotLoginRunner()
    @State private var expertMode = ExpertMode.shared

    var body: some View {
        VStack(spacing: 22) {
            self.header
            self.phaseView
            self.footer
        }
        .padding(28)
        .frame(width: 460)
        .background(self.theme.colors.background)
        .task {
            self.runner.start()
        }
        .onChange(of: self.runner.state) { _, newValue in
            if case .completed = newValue {
                self.onSuccess()
            }
        }
    }

    // MARK: - Header

    private var header: some View {
        VStack(spacing: 6) {
            Image(systemName: "person.crop.circle.badge.checkmark")
                .font(.system(size: 36, weight: .semibold))
                .foregroundStyle(self.theme.colors.primary)
            Text(L10n.tr("setup.cloud_auth.sheet.title"))
                .bobeTextStyle(.setupHeading)
                .foregroundStyle(self.theme.colors.text)
            Text(L10n.tr("setup.cloud_auth.sheet.subtitle"))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)
        }
    }

    // MARK: - Body (phase-driven)

    @ViewBuilder
    private var phaseView: some View {
        switch self.runner.state {
        case .idle, .preparing:
            self.preparingBody
        case let .awaitingUser(url, code), let .polling(url, code):
            self.codeBody(url: url, code: code)
        case .completed:
            self.successBody
        case let .failed(message):
            self.failedBody(message: message)
        case .canceled:
            self.canceledBody
        }
    }

    private var preparingBody: some View {
        VStack(spacing: 10) {
            ProgressView()
                .scaleEffect(0.9)
            Text(L10n.tr("setup.cloud_auth.sheet.preparing"))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
        }
        .frame(maxWidth: .infinity, minHeight: 160)
    }

    private func codeBody(url: URL, code: String) -> some View {
        VStack(spacing: 16) {
            // Step 1: open browser.
            VStack(spacing: 6) {
                Text(L10n.tr("setup.cloud_auth.sheet.step1"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                Button {
                    self.runner.openVerificationURL()
                } label: {
                    HStack(spacing: 6) {
                        Image(systemName: "arrow.up.right.square")
                        Text(L10n.tr("setup.cloud_auth.sheet.open_browser"))
                    }
                }
                .bobeButton(.primary, size: .regular)
                Text(url.absoluteString)
                    .font(.system(size: 11, design: .monospaced))
                    .foregroundStyle(self.theme.colors.textMuted)
                    .textSelection(.enabled)
            }

            // Step 2: enter code.
            VStack(spacing: 8) {
                Text(L10n.tr("setup.cloud_auth.sheet.step2"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
                HStack(spacing: 10) {
                    Text(code)
                        .font(.system(size: 28, weight: .bold, design: .monospaced))
                        .kerning(2)
                        .minimumScaleFactor(0.7)
                        .lineLimit(1)
                        .foregroundStyle(self.theme.colors.text)
                        .padding(.horizontal, 18)
                        .padding(.vertical, 10)
                        .background(
                            RoundedRectangle(cornerRadius: 10)
                                .fill(self.theme.colors.surface)
                                .overlay(
                                    RoundedRectangle(cornerRadius: 10)
                                        .stroke(self.theme.colors.border, lineWidth: 1)
                                )
                        )
                        .textSelection(.enabled)
                    Button {
                        self.runner.copyCode()
                    } label: {
                        HStack(spacing: 4) {
                            Image(systemName: self.runner.didCopyCode ? "checkmark" : "doc.on.doc")
                            Text(
                                self.runner.didCopyCode
                                    ? L10n.tr("setup.cloud_auth.sheet.copied")
                                    : L10n.tr("setup.cloud_auth.sheet.copy")
                            )
                        }
                    }
                    .bobeButton(.secondary, size: .small)
                }
            }

            // Live status — "Waiting for authorization..." once the CLI
            // polls. Keeps the user oriented without us having to peek at
            // the daemon's auth status.
            HStack(spacing: 6) {
                ProgressView()
                    .scaleEffect(0.7)
                Text(L10n.tr("setup.cloud_auth.sheet.waiting"))
                    .bobeTextStyle(.helper)
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .padding(.top, 4)
        }
    }

    private var successBody: some View {
        VStack(spacing: 10) {
            Image(systemName: "checkmark.seal.fill")
                .font(.system(size: 36))
                .foregroundStyle(self.theme.colors.secondary)
            Text(L10n.tr("setup.cloud_auth.sheet.success"))
                .bobeTextStyle(.setupHeading)
                .foregroundStyle(self.theme.colors.text)
        }
        .frame(maxWidth: .infinity, minHeight: 140)
    }

    private func failedBody(message: String) -> some View {
        VStack(spacing: 10) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 32))
                .foregroundStyle(self.theme.colors.primary)
            Text(L10n.tr("setup.cloud_auth.sheet.failed_title"))
                .bobeTextStyle(.setupHeading)
                .foregroundStyle(self.theme.colors.text)
            Text(Self.humanize(failure: message))
                .bobeTextStyle(.helper)
                .foregroundStyle(self.theme.colors.textMuted)
                .multilineTextAlignment(.center)
                .frame(maxWidth: 380)
            if self.expertMode.isEnabled {
                DisclosureGroup(L10n.tr("setup.cloud_auth.sheet.error_details")) {
                    Text(message)
                        .font(.system(size: 10, design: .monospaced))
                        .foregroundStyle(self.theme.colors.textMuted)
                        .multilineTextAlignment(.leading)
                        .textSelection(.enabled)
                        .padding(.top, 4)
                }
                .bobeTextStyle(.helper)
                .frame(maxWidth: 380, alignment: .leading)
            }
        }
        .frame(maxWidth: .infinity, minHeight: 160)
    }

    /// Translates raw device-flow / daemon error strings into a single
    /// friendly sentence. The raw text is preserved behind a disclosure
    /// for Expert users; everyone else sees only the humanized line.
    static func humanize(failure raw: String) -> String {
        let lower = raw.lowercased()
        if lower.contains("expired") || lower.contains("expired_token") {
            return L10n.tr("setup.cloud_auth.sheet.error.expired")
        }
        if lower.contains("access_denied") || lower.contains("denied") {
            return L10n.tr("setup.cloud_auth.sheet.error.denied")
        }
        if lower.contains("slow_down") {
            return L10n.tr("setup.cloud_auth.sheet.error.slow_down")
        }
        if lower.contains("network") || lower.contains("offline") || lower.contains("timeout") {
            return L10n.tr("setup.cloud_auth.sheet.error.network")
        }
        if lower.contains("pkce") || lower.contains("device_code") || self.containsStatusCode(lower, "400") {
            return L10n.tr("setup.cloud_auth.sheet.error.handshake")
        }
        return L10n.tr("setup.cloud_auth.sheet.error.generic")
    }

    /// Word-boundary-style substring check for a numeric status code inside a
    /// free-form error string. Prevents accidental matches like "port 24000"
    /// or "timeout 4000ms" from being read as an HTTP 400.
    private static func containsStatusCode(_ haystack: String, _ code: String) -> Bool {
        let chars = Array(haystack)
        let codeChars = Array(code)
        guard codeChars.count <= chars.count else { return false }
        for i in 0 ... chars.count - codeChars.count {
            let slice = Array(chars[i ..< i + codeChars.count])
            guard slice == codeChars else { continue }
            let prevOk = i == 0 || !chars[i - 1].isNumber
            let nextIdx = i + codeChars.count
            let nextOk = nextIdx == chars.count || !chars[nextIdx].isNumber
            if prevOk, nextOk { return true }
        }
        return false
    }

    private var canceledBody: some View {
        VStack(spacing: 10) {
            Image(systemName: "xmark.circle")
                .font(.system(size: 32))
                .foregroundStyle(self.theme.colors.textMuted)
            Text(L10n.tr("setup.cloud_auth.sheet.canceled"))
                .bobeTextStyle(.setupHeading)
                .foregroundStyle(self.theme.colors.text)
        }
        .frame(maxWidth: .infinity, minHeight: 140)
    }

    // MARK: - Footer

    @ViewBuilder
    private var footer: some View {
        switch self.runner.state {
        case .idle, .preparing, .awaitingUser, .polling:
            Button(L10n.tr("setup.cloud_auth.sheet.cancel")) {
                self.runner.cancel()
                self.onClose()
            }
            .bobeButton(.ghost, size: .small)
            .keyboardShortcut(.cancelAction)
        case .completed:
            Button(L10n.tr("setup.cloud_auth.sheet.done"), action: self.onClose)
                .bobeButton(.primary, size: .regular)
                .keyboardShortcut(.defaultAction)
        case .failed:
            HStack(spacing: 10) {
                Button(L10n.tr("setup.cloud_auth.sheet.try_again")) {
                    self.runner.reset()
                    self.runner.start()
                }
                .bobeButton(.primary, size: .regular)
                if self.expertMode.isEnabled {
                    Button(L10n.tr("setup.cloud_auth.sheet.use_terminal")) {
                        CopilotSignIn.openLogin(cliPath: nil)
                        self.onClose()
                    }
                    .bobeButton(.secondary, size: .small)
                }
                Button(L10n.tr("setup.cloud_auth.sheet.cancel"), action: self.onClose)
                    .bobeButton(.ghost, size: .small)
                    .keyboardShortcut(.cancelAction)
            }
        case .canceled:
            HStack(spacing: 10) {
                Button(L10n.tr("setup.cloud_auth.sheet.try_again")) {
                    self.runner.reset()
                    self.runner.start()
                }
                .bobeButton(.primary, size: .regular)
                Button(L10n.tr("setup.cloud_auth.sheet.cancel"), action: self.onClose)
                    .bobeButton(.ghost, size: .small)
                    .keyboardShortcut(.cancelAction)
            }
        }
    }
}
