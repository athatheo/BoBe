import SwiftUI

/// MCP Servers management panel.
/// Based on MCPServersSettings.tsx with connection status, excluded tools tags, delete confirmation.
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

    private var selectedServer: MCPServer? { servers.first { $0.id == selectedId } }

    var body: some View {
        ThemedSplitPane(leftWidth: 300) {
            // Left pane
            VStack(alignment: .leading, spacing: 0) {
                SettingsPaneHeader(title: "MCP Servers") { isAdding = true }
                    .padding(.bottom, 12)

                if isLoading && servers.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading MCP servers...")
                            .font(.system(size: 12))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 20)
                } else if servers.isEmpty && !isLoading && !isAdding {
                    VStack(spacing: 8) {
                        Image(systemName: "server.rack")
                            .font(.system(size: 28))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("No MCP servers")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("Add servers to extend BoBe's capabilities")
                            .font(.system(size: 11))
                            .foregroundStyle(theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 32)
                } else {
                    ScrollView {
                        LazyVStack(spacing: 4) {
                            ForEach(servers) { server in
                                BobeSelectableRow(
                                    isSelected: selectedId == server.id,
                                    action: {
                                        selectedId = server.id
                                        isAdding = false
                                    },
                                    content: {
                                    HStack(spacing: 8) {
                                        Image(systemName: server.connected ? "wifi" : "wifi.slash")
                                            .foregroundStyle(server.connected ? theme.colors.secondary : theme.colors.primary)
                                            .font(.system(size: 12))

                                        VStack(alignment: .leading, spacing: 3) {
                                            Text(server.name)
                                                .font(.system(size: 13, weight: .semibold))
                                            Text("\(server.command) • \(server.toolCount) tools")
                                                .font(.system(size: 11))
                                                .foregroundStyle(theme.colors.textMuted)
                                        }

                                        if server.error != nil {
                                            Image(systemName: "exclamationmark.triangle.fill")
                                                .foregroundStyle(theme.colors.tertiary)
                                                .font(.system(size: 10))
                                        }
                                    }
                                })
                            }
                        }
                    }
                    .background(theme.colors.background)
                }
            }
            .frame(minWidth: 220, idealWidth: 300)
            .frame(maxHeight: .infinity, alignment: .top)
            .padding(.horizontal, 12)
            .padding(.top, 12)
        } right: {
            // Right pane
            if isAdding {
                addServerForm
            } else if let server = selectedServer {
                MCPServerDetail(
                    server: server,
                    deleteConfirm: $deleteConfirm,
                    isReconnecting: $isReconnecting,
                    addExcludedText: $addExcludedText,
                    error: $error,
                    onReconnect: reconnectServer,
                    onRemove: removeServer,
                    onAddExcluded: addExcludedTool,
                    onRemoveExcluded: removeExcludedTool
                )
            } else {
                VStack(spacing: 8) {
                    Image(systemName: "server.rack")
                        .font(.system(size: 28))
                        .foregroundStyle(theme.colors.textMuted)
                    Text("Select a server or add a new one")
                        .font(.system(size: 13))
                        .foregroundStyle(theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }
        }
        .task { await loadServers() }
    }

    private var addServerForm: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 14) {
                Text("New MCP Server")
                    .font(.headline)
                    .foregroundStyle(theme.colors.text)

                VStack(alignment: .leading, spacing: 4) {
                    Text("Server Name")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeTextField(placeholder: "e.g. filesystem", text: $newName)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Command")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeTextField(placeholder: "e.g. npx or /usr/local/bin/...", text: $newCommand)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Arguments")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeTextField(placeholder: "e.g. -y @modelcontextprotocol/...", text: $newArgs)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Environment Variables (JSON)")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    CodeEditor(text: $newEnv, theme: theme, fontSize: 12)
                        .frame(height: 80)
                        .background(
                            RoundedRectangle(cornerRadius: 6)
                                .fill(theme.colors.surface)
                                .stroke(theme.colors.border, lineWidth: 1)
                        )
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Excluded Tools")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    BobeTextField(placeholder: "e.g. update_event, delete_event", text: $newExcluded)
                    Text("Tool names to hide from BoBe (server still exposes them)")
                        .font(.system(size: 10))
                        .foregroundStyle(theme.colors.textMuted)
                }

                HStack {
                    Button("Cancel") {
                        isAdding = false
                        clearForm()
                    }
                    .bobeButton(.secondary, size: .small)

                    Spacer()

                    Button("Add & Connect") { addServer() }
                        .bobeButton(.primary, size: .small)
                        .disabled(newName.isEmpty || newCommand.isEmpty)
                }

                if let error {
                    Text(error).font(.caption).foregroundStyle(theme.colors.primary)
                }
            }
            .padding(.horizontal, 16)
            .padding(.top, 12)
        }
    }

    // MARK: - Actions

    private func loadServers() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listMCPServers()
            servers = resp.servers
        } catch { self.error = error.localizedDescription }
    }

    private func addServer() {
        let args = newArgs.isEmpty ? nil : newArgs.split(separator: " ").map(String.init)
        let env: [String: String]? = {
            guard !newEnv.isEmpty, let data = newEnv.data(using: .utf8) else { return nil }
            return try? JSONDecoder().decode([String: String].self, from: data)
        }()
        let excluded = newExcluded.isEmpty ? nil : newExcluded.split(separator: ",").map { $0.trimmingCharacters(in: .whitespaces) }

        Task {
            do {
                _ = try await DaemonClient.shared.createMCPServer(
                    MCPServerCreateRequest(name: newName, command: newCommand, args: args, env: env, excludedTools: excluded)
                )
                clearForm()
                isAdding = false
                await loadServers()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func reconnectServer(_ server: MCPServer) {
        isReconnecting = true
        Task {
            defer { isReconnecting = false }
            do {
                _ = try await DaemonClient.shared.reconnectMCPServer(server.name)
                await loadServers()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func removeServer(_ server: MCPServer) {
        Task {
            do {
                try await DaemonClient.shared.deleteMCPServer(server.name)
                servers.removeAll { $0.id == server.id }
                if selectedId == server.id { selectedId = nil }
            } catch { self.error = error.localizedDescription }
        }
    }

    private func addExcludedTool(server: MCPServer) {
        guard !addExcludedText.isEmpty else { return }
        var updated = server.excludedTools
        updated.append(addExcludedText.trimmingCharacters(in: .whitespaces))
        addExcludedText = ""
        Task {
            do {
                _ = try await DaemonClient.shared.updateMCPServer(
                    server.name,
                    MCPServerUpdateRequest(excludedTools: updated)
                )
                await loadServers()
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
                await loadServers()
            } catch { self.error = error.localizedDescription }
        }
    }

    private func clearForm() {
        newName = ""
        newCommand = ""
        newArgs = ""
        newEnv = ""
        newExcluded = ""
    }
}
