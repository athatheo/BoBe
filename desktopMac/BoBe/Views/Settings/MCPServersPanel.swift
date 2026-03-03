import SwiftUI

/// Full-document MCP configuration editor (`mcp.json`) with validation and save flow.
struct MCPServersPanel: View {
    @State private var rawJson = ""
    @State private var servers: [MCPServer] = []
    @State private var isLoading = false
    @State private var isValidating = false
    @State private var isSaving = false
    @State private var status: String?
    @State private var error: String?
    @State private var lastValidSecretMap: [String: [String]]?
    @Environment(\.theme) private var theme

    private var isJsonEmpty: Bool {
        self.rawJson.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("MCP Configuration")
                    .font(.title2.bold())
                    .foregroundStyle(self.theme.colors.text)

                // swiftlint:disable:next line_length
                Text("Edit the full mcp.json document. Save persists the config and connects servers. Secret values are stored securely in the system keychain.")
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)

                CodeEditor(text: self.$rawJson, theme: self.theme, fontSize: 12)
                    .frame(height: 260)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(self.theme.colors.surface)
                            .stroke(self.theme.colors.border, lineWidth: 1)
                    )

                HStack(spacing: 8) {
                    Button("Reload") { self.reloadConfig() }
                        .bobeButton(.secondary, size: .small)
                        .disabled(self.isLoading || self.isValidating || self.isSaving)

                    Button {
                        self.validateConfig()
                    } label: {
                        HStack(spacing: 4) {
                            if self.isValidating {
                                BobeSpinner(size: 12)
                            }
                            Text(self.isValidating ? "Validating..." : "Validate")
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
                            Text(self.isSaving ? "Saving..." : "Save")
                        }
                    }
                    .bobeButton(.primary, size: .small)
                    .disabled(self.isLoading || self.isValidating || self.isSaving || self.isJsonEmpty)

                    Spacer()

                    Button("Reset") { self.resetConfig() }
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

                Divider()

                self.discoverySection
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
            .padding(.bottom, 16)
        }
        .task { await self.loadConfig() }
    }

    @ViewBuilder
    private var discoverySection: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Discovered Servers")
                .font(.headline)
                .foregroundStyle(self.theme.colors.text)

            if self.isLoading {
                HStack(spacing: 8) {
                    BobeSpinner(size: 12)
                    Text("Loading MCP config...")
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                }
            } else if self.servers.isEmpty {
                Text("No MCP servers configured.")
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)
            } else {
                LazyVStack(alignment: .leading, spacing: 6) {
                    ForEach(self.servers) { server in
                        VStack(alignment: .leading, spacing: 4) {
                            HStack(spacing: 8) {
                                Text(server.name)
                                    .font(.system(size: 12, weight: .semibold))
                                Text(server.connected ? "connected" : "disconnected")
                                    .font(.system(size: 10, weight: .medium))
                                    .foregroundStyle(server.connected ? self.theme.colors.secondary : self.theme.colors.primary)
                                Text("\(server.toolCount) tools")
                                    .font(.system(size: 10))
                                    .foregroundStyle(self.theme.colors.textMuted)
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
                }
            }
        }
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
            self.error = "MCP JSON must not be empty."
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
                    self.status = "Validation passed (\(response.serverCount) server(s))."
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
            self.error = "MCP JSON must not be empty."
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
