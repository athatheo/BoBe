import SwiftUI

/// MCP Servers management panel.
struct MCPServersPanel: View {
    @State private var servers: [MCPServer] = []
    @State private var selectedId: String?
    @State private var isAdding = false
    @State private var isLoading = false
    @State private var isReconnecting = false
    @State private var deleteConfirm = false
    @State private var newName = ""
    @State private var newCommand = ""
    @State private var newArgs = ""
    @State private var newEnv = ""
    @State private var newExcluded = ""
    @State private var addExcludedText = ""
    @State private var error: String?
    @Environment(\.theme) private var theme

    private var selectedServer: MCPServer? {
        self.servers.first { $0.id == self.selectedId }
    }

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "MCP Servers") { self.isAdding = true }
                    .padding(.bottom, 12)

                if self.isLoading, self.servers.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading MCP servers...")
                            .font(.system(size: 12))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if self.servers.isEmpty, !self.isLoading, !self.isAdding {
                    VStack(spacing: 8) {
                        Image(systemName: "server.rack")
                            .font(.system(size: 28))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("No MCP servers")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(self.theme.colors.textMuted)
                        Text("Add servers to extend BoBe's capabilities")
                            .font(.system(size: 11))
                            .foregroundStyle(self.theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(self.servers) { server in
                                BobeSelectableRow(
                                    isSelected: self.selectedId == server.id,
                                    action: {
                                        self.selectedId = server.id
                                        self.isAdding = false
                                    },
                                    content: {
                                        HStack(spacing: 8) {
                                            Image(systemName: server.connected ? "wifi" : "wifi.slash")
                                                .foregroundStyle(server.connected ? self.theme.colors.secondary : self.theme.colors.primary)
                                                .font(.system(size: 12))

                                            VStack(alignment: .leading, spacing: 3) {
                                                Text(server.name)
                                                    .font(.system(size: 13, weight: .semibold))
                                                Text("\(server.command) • \(server.toolCount) tools")
                                                    .font(.system(size: 11))
                                                    .foregroundStyle(self.theme.colors.textMuted)
                                            }

                                            if server.error != nil {
                                                Image(systemName: "exclamationmark.triangle.fill")
                                                    .foregroundStyle(self.theme.colors.tertiary)
                                                    .font(.system(size: 10))
                                            }
                                        }
                                    }
                                )
                            }
                        }
                    }
                    .background(self.theme.colors.background)
                }
            }
            .frame(minWidth: 220, idealWidth: 300)
            .frame(maxHeight: .infinity, alignment: .top)
            .padding(.horizontal, 12)
            .padding(.top, 12)
        } right: {
            if self.isAdding {
                self.addServerForm
            } else if let server = selectedServer {
                MCPServerDetail(
                    server: server,
                    deleteConfirm: self.$deleteConfirm,
                    isReconnecting: self.$isReconnecting,
                    addExcludedText: self.$addExcludedText,
                    error: self.$error,
                    onReconnect: self.reconnectServer,
                    onRemove: self.removeServer,
                    onAddExcluded: self.addExcludedTool,
                    onRemoveExcluded: self.removeExcludedTool
                )
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "server.rack")
                        .font(.system(size: 28))
                        .foregroundStyle(self.theme.colors.textMuted)
                    Text("Select a server or add a new one")
                        .font(.system(size: 13))
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .task { await self.loadServers() }
    }

    private var addServerForm: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("New MCP Server")
                    .font(.headline)
                    .foregroundStyle(self.theme.colors.text)

                VStack(alignment: .leading, spacing: 4) {
                    Text("Server Name")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(self.theme.colors.text)
                    BobeTextField(placeholder: "e.g. filesystem", text: self.$newName)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Command")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(self.theme.colors.text)
                    BobeTextField(placeholder: "e.g. npx or /usr/local/bin/...", text: self.$newCommand)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Arguments")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(self.theme.colors.text)
                    BobeTextField(placeholder: "e.g. -y @modelcontextprotocol/...", text: self.$newArgs)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Environment Variables (JSON)")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(self.theme.colors.text)
                    CodeEditor(text: self.$newEnv, theme: self.theme, fontSize: 12)
                        .frame(height: 80)
                        .background(
                            RoundedRectangle(cornerRadius: 6)
                                .fill(self.theme.colors.surface)
                                .stroke(self.theme.colors.border, lineWidth: 1)
                        )
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Excluded Tools")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(self.theme.colors.text)
                    BobeTextField(placeholder: "e.g. update_event, delete_event", text: self.$newExcluded)
                    Text("Tool names to hide from BoBe (server still exposes them)")
                        .font(.system(size: 10))
                        .foregroundStyle(self.theme.colors.textMuted)
                }

                HStack {
                    Button("Cancel") {
                        self.isAdding = false
                        self.clearForm()
                    }
                    .bobeButton(.secondary, size: .small)

                    Spacer()

                    Button("Add & Connect") { self.addServer() }
                        .bobeButton(.primary, size: .small)
                        .disabled(self.newName.isEmpty || self.newCommand.isEmpty)
                }

                if let error {
                    Text(error).font(.caption).foregroundStyle(self.theme.colors.primary)
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
        }
    }

    // MARK: - Actions

    private func loadServers() async {
        self.isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listMCPServers()
            self.servers = resp.servers
        } catch { self.error = error.localizedDescription }
    }

    private func addServer() {
        let args = self.newArgs.isEmpty ? nil : self.newArgs.split(separator: " ").map(String.init)
        let env: [String: String]? = {
            guard !self.newEnv.isEmpty, let data = newEnv.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode([String: String].self, from: data)
        }()
        let excluded = self.newExcluded.isEmpty ? nil : self.newExcluded.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }

        Task {
            do {
                _ = try await DaemonClient.shared.createMCPServer(
                    MCPServerCreateRequest(name: self.newName, command: self.newCommand, args: args, env: env, excludedTools: excluded)
                )
                self.clearForm()
                self.isAdding = false
                await self.loadServers()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func reconnectServer(_ server: MCPServer) {
        self.isReconnecting = true
        Task {
            defer { isReconnecting = false }
            do {
                _ = try await DaemonClient.shared.reconnectMCPServer(server.name)
                await self.loadServers()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func removeServer(_ server: MCPServer) {
        Task {
            do {
                try await DaemonClient.shared.deleteMCPServer(server.name)
                self.servers.removeAll { $0.id == server.id }
                if self.selectedId == server.id { self.selectedId = nil }
            } catch { self.error = error.localizedDescription }
        }
    }

    private func addExcludedTool(server: MCPServer) {
        guard !self.addExcludedText.isEmpty else { return }
        var updated = server.excludedTools
        updated.append(self.addExcludedText.trimmingCharacters(in: .whitespaces))
        self.addExcludedText = ""
        Task {
            do {
                _ = try await DaemonClient.shared.updateMCPServer(
                    server.name,
                    MCPServerUpdateRequest(excludedTools: updated)
                )
                await self.loadServers()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func removeExcludedTool(server: MCPServer, tool: String) {
        var updated = server.excludedTools
        updated.removeAll { $0 == tool }
        Task {
            do {
                _ = try await DaemonClient.shared.updateMCPServer(
                    server.name,
                    MCPServerUpdateRequest(excludedTools: updated)
                )
                await self.loadServers()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func clearForm() {
        self.newName = ""
        self.newCommand = ""
        self.newArgs = ""
        self.newEnv = ""
        self.newExcluded = ""
    }
}
