import SwiftUI

struct MCPServersPanel: View {
    @State private var rawJson = ""
    @State private var servers: [MCPServer] = []
    @State private var isLoading = false
    @State private var isValidating = false
    @State private var isSaving = false
    @State private var status: String?
    @State private var error: String?
    @State private var lastValidSecretMap: [String: [String]]?
    @State private var expertMode = ExpertMode.shared
    @State private var store = SettingsStore.shared
    @State private var restartBannerDismissed = false
    @Environment(\.theme) private var theme

    /// Daemon claims hot-apply but the MCP map is captured at boot — force a
    /// restart banner locally so users know the toggle takes effect on the
    /// next launch, not mid-session.
    private static let deferToRestartFields: Set = ["mcp_enabled"]

    private var visibleRestartFields: Set<String> {
        self.restartBannerDismissed ? [] : self.store.restartFields
    }

    private var isJsonEmpty: Bool {
        self.rawJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text(L10n.tr("settings.mcp.title"))
                    .font(.title2.bold())
                    .foregroundStyle(self.theme.colors.text)

                Text(L10n.tr("settings.mcp.description"))
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)

                if !self.visibleRestartFields.isEmpty {
                    RestartRequiredBanner(
                        fields: self.visibleRestartFields,
                        onDismiss: { self.restartBannerDismissed = true }
                    )
                }

                // Settings-side errors (mcp_enabled toggle PATCH failures)
                // surface here. JSON-editor errors render inline below the
                // editor — they're conceptually different surfaces. Showing
                // both at their natural locations is clearer than collapsing
                // them into one banner that hides the cause.
                if let storeError = self.store.error {
                    SettingsErrorBanner(message: storeError)
                }
                if let savedMessage = self.store.savedMessage {
                    SettingsSavedToast(message: savedMessage)
                }

                self.enableToggleRow

                if !self.expertMode.isEnabled {
                    Text(L10n.tr("settings.mcp.novice.lede"))
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                        .padding(.bottom, 4)
                }

                if self.expertMode.isEnabled {
                    CodeEditor(text: self.$rawJson, theme: self.theme, fontSize: 12)
                        .frame(height: 260)
                        .background(
                            RoundedRectangle(cornerRadius: 8)
                                .fill(self.theme.colors.surface)
                                .stroke(self.theme.colors.border, lineWidth: 1)
                        )

                    HStack(spacing: 8) {
                        Button(L10n.tr("settings.mcp.action.reload")) { self.reloadConfig() }
                            .bobeButton(.secondary, size: .small)
                            .disabled(self.isLoading || self.isValidating || self.isSaving)

                        Button {
                            self.validateConfig()
                        } label: {
                            HStack(spacing: 4) {
                                if self.isValidating {
                                    BobeSpinner(size: 12)
                                }
                                Text(
                                    self.isValidating
                                        ? L10n.tr("settings.mcp.action.validating")
                                        : L10n.tr("settings.mcp.action.validate")
                                )
                            }
                        }
                        .bobeButton(.secondary, size: .small)
                        .disabled(self.isLoading || self.isValidating || self.isSaving || self.isJsonEmpty)

                        Button {
                            self.saveConfig()
                        } label: {
                            HStack(spacing: 4) {
                                if self.isSaving {
                                    BobeSpinner(size: 12)
                                }
                                Text(
                                    self.isSaving
                                        ? L10n.tr("settings.shared.action.saving")
                                        : L10n.tr("settings.shared.action.save")
                                )
                            }
                        }
                        .bobeButton(.primary, size: .small)
                        .disabled(self.isLoading || self.isValidating || self.isSaving || self.isJsonEmpty)

                        Spacer()

                        Button(L10n.tr("settings.mcp.action.reset")) { self.resetConfig() }
                            .bobeButton(.destructive, size: .small)
                            .disabled(self.isLoading || self.isValidating || self.isSaving)
                    }

                    if let status {
                        Text(status)
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.secondary)
                    }

                    if let error {
                        Text(error)
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.primary)
                    }
                } else {
                    self.noviceEditorHidden
                }

                Divider()

                self.discoverySection
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
            .padding(.bottom, 16)
        }
        .task {
            await self.loadConfig()
            await self.store.loadIfNeeded()
        }
        .onDisappear { self.store.cancelToast() }
    }

    /// Top-of-panel switch for MCP at the daemon level. Lives here (rather
    /// than its old home in Advanced) so the toggle that gates everything
    /// below it is the first control the user sees.
    private var enableToggleRow: some View {
        SettingsRow(
            label: L10n.tr("settings.mcp.enable"),
            description: L10n.tr("settings.mcp.enable.description")
        ) {
            BobeToggle(isOn: self.mcpEnabledBinding)
        }
    }

    private var mcpEnabledBinding: Binding<Bool> {
        // Routes through SettingsStore so the toggle change syncs with other
        // panels and the restart-required banner state. The store also
        // tracks `mcp_enabled` in its `deferToRestartFields` so the banner
        // fires immediately on touch.
        self.store.binding(\.mcpEnabled, fallback: false, touched: "mcp_enabled")
    }

    /// Shown when Expert mode is off. Explains that there's a powerful
    /// raw-JSON editor available and points the user at the toggle so they
    /// know the door isn't locked, just hidden.
    private var noviceEditorHidden: some View {
        VStack(alignment: .leading, spacing: 6) {
            HStack(spacing: 6) {
                Image(systemName: "wrench.and.screwdriver")
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)
                Text(L10n.tr("settings.mcp.novice.editor_hidden"))
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
        }
        .padding(12)
        .frame(maxWidth: .infinity, alignment: .leading)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(self.theme.colors.surface)
                .overlay(
                    RoundedRectangle(cornerRadius: 8)
                        .stroke(self.theme.colors.border.opacity(0.6), lineWidth: 1)
                )
        )
    }

    private var discoverySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text(L10n.tr("settings.mcp.discovery.title"))
                .font(.headline)
                .foregroundStyle(self.theme.colors.text)

            if self.isLoading {
                HStack(spacing: 8) {
                    BobeSpinner(size: 12)
                    Text(L10n.tr("settings.mcp.loading"))
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                }
            } else if self.servers.isEmpty {
                Text(L10n.tr("settings.mcp.discovery.empty"))
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)
            } else {
                LazyVStack(alignment: .leading, spacing: 6) {
                    ForEach(self.servers) { server in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack(spacing: 8) {
                                Text(server.name)
                                    .font(.system(size: 12, weight: .semibold))
                                Text(
                                    server.enabled
                                        ? L10n.tr("settings.mcp.discovery.status.enabled")
                                        : L10n.tr("settings.mcp.discovery.status.disabled")
                                )
                                .font(.system(size: 10, weight: .medium))
                                .foregroundStyle(server.enabled ? self.theme.colors.secondary : self.theme.colors.textMuted)
                                self.statusBadge(for: server)
                            }
                            .foregroundStyle(self.theme.colors.text)

                            if !server.command.isEmpty {
                                Text("\(server.command) \(server.args.joined(separator: " "))")
                                    .font(.system(size: 10, design: .monospaced))
                                    .foregroundStyle(self.theme.colors.textMuted)
                            }

                            if let serverError = server.error, !serverError.isEmpty {
                                Text(serverError)
                                    .font(.system(size: 10))
                                    .foregroundStyle(self.theme.colors.primary)
                            }
                        }
                        .padding(8)
                        .frame(maxWidth: .infinity, alignment: .leading)
                        .background(RoundedRectangle(cornerRadius: 6).fill(self.theme.colors.surface))
                    }

                    // Status comes from `session.mcp.list` RPC; absent until session spawns.
                    if self.servers.contains(where: { $0.status == nil }) {
                        Text(L10n.tr("settings.mcp.discovery.runtime_state_pending"))
                            .font(.system(size: 10).italic())
                            .foregroundStyle(self.theme.colors.textMuted.opacity(0.8))
                            .padding(.top, 4)
                    }
                }
            }
        }
    }

    @ViewBuilder
    private func statusBadge(for server: MCPServer) -> some View {
        if let status = server.status {
            switch status {
            case McpServerStatusWire.connected:
                self.badge(
                    text: L10n.tr("settings.mcp.runtime.connected"),
                    color: self.theme.colors.secondary
                )
            case McpServerStatusWire.failed:
                self.badge(
                    text: L10n.tr("settings.mcp.runtime.failed"),
                    color: self.theme.colors.primary
                )
            case McpServerStatusWire.needsAuth:
                self.badge(
                    text: L10n.tr("settings.mcp.runtime.needs_auth"),
                    color: self.theme.colors.tertiary
                )
            case McpServerStatusWire.pending:
                self.badge(
                    text: L10n.tr("settings.mcp.runtime.pending"),
                    color: self.theme.colors.textMuted
                )
            case McpServerStatusWire.disabled, McpServerStatusWire.notConfigured:
                EmptyView()
            default:
                self.badge(text: status, color: self.theme.colors.textMuted)
            }
        }
    }

    private func badge(text: String, color: Color) -> some View {
        Text(text)
            .font(.system(size: 9, weight: .medium))
            .padding(.horizontal, 6)
            .padding(.vertical, 2)
            .background(
                Capsule()
                    .fill(color.opacity(0.15))
                    .overlay(Capsule().stroke(color.opacity(0.5), lineWidth: 0.5))
            )
            .foregroundStyle(color)
    }

    private func loadConfig() async {
        self.isLoading = true
        self.status = nil
        self.error = nil
        defer { self.isLoading = false }

        do {
            let response = try await DaemonClient.shared.getMCPConfig()
            self.rawJson = response.rawJson
            self.servers = response.servers
        } catch {
            self.error = error.localizedDescription
        }
    }

    private func reloadConfig() {
        Task { await self.loadConfig() }
    }

    private func validateConfig() {
        guard !self.rawJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            self.error = L10n.tr("settings.mcp.error.empty_json")
            return
        }

        self.isValidating = true
        self.status = nil
        self.error = nil
        let capturedJson = self.rawJson
        let request = MCPConfigMutationRequest(
            rawJson: capturedJson,
            secretKeys: self.extractSecretKeyMap(from: capturedJson)
        )

        Task {
            defer { self.isValidating = false }
            do {
                let response = try await DaemonClient.shared.validateMCPConfig(request)
                if response.valid {
                    self.rawJson = response.normalizedJson
                    self.status = L10n.tr("settings.mcp.status.validation_passed_format", response.serverCount)
                } else {
                    self.error = response.errors.joined(separator: "\n")
                }
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func saveConfig() {
        guard !self.rawJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty else {
            self.error = L10n.tr("settings.mcp.error.empty_json")
            return
        }

        self.isSaving = true
        self.status = nil
        self.error = nil
        let capturedJson = self.rawJson
        let request = MCPConfigMutationRequest(
            rawJson: capturedJson,
            secretKeys: self.extractSecretKeyMap(from: capturedJson)
        )

        Task {
            defer { self.isSaving = false }
            do {
                let response = try await DaemonClient.shared.saveMCPConfig(request)
                self.rawJson = response.rawJson
                self.servers = response.servers
                self.status = response.message
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func resetConfig() {
        self.status = nil
        self.error = nil
        self.isSaving = true

        Task {
            defer { self.isSaving = false }
            do {
                let response = try await DaemonClient.shared.resetMCPConfig()
                self.rawJson = response.rawJson
                self.servers = []
                self.status = response.message
            } catch {
                self.error = error.localizedDescription
            }
        }
    }

    private func extractSecretKeyMap(from json: String) -> [String: [String]]? {
        guard let data = json.data(using: .utf8),
              let doc = try? JSONDecoder().decode(InputDoc.self, from: data)
        else {
            return self.lastValidSecretMap
        }

        let allServers = doc.mcpServers ?? doc.servers ?? [:]
        var secretMap: [String: [String]] = [:]

        for (serverName, entry) in allServers {
            guard let env = entry.env else {
                continue
            }

            let keys = env.compactMap { key, value -> String? in
                let upper = key.uppercased()
                if value.isEmpty || value.contains("${") || value.hasPrefix("bobe-secret://") {
                    return nil
                }
                if upper.contains("SECRET")
                    || upper.contains("TOKEN")
                    || upper.contains("PASSWORD")
                    || upper.hasSuffix("API_KEY") {
                    return key
                }
                return nil
            }

            if !keys.isEmpty {
                secretMap[serverName] = keys.sorted()
            }
        }

        let result = secretMap.isEmpty ? nil : secretMap
        self.lastValidSecretMap = result
        return result
    }
}

private struct InputServerEntry: Decodable {
    let env: [String: String]?
}

private struct InputDoc: Decodable {
    let mcpServers: [String: InputServerEntry]?
    let servers: [String: InputServerEntry]?
}
