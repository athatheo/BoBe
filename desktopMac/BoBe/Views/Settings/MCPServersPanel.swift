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
        HSplitView {
            // Left pane
            VStack(alignment: .leading, spacing: 8) {
                HStack {
                    Text("MCP Servers")
                        .font(.headline)
                        .foregroundStyle(theme.colors.text)
                    Spacer()
                    Button { isAdding = true } label: {
                        Image(systemName: "plus.circle.fill")
                    }
                    .buttonStyle(.plain)
                }

                if isLoading && servers.isEmpty {
                    HStack(spacing: 8) {
                        ProgressView().controlSize(.small)
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
                    List(selection: $selectedId) {
                        ForEach(servers) { server in
                            HStack(spacing: 8) {
                                Image(systemName: server.connected ? "wifi" : "wifi.slash")
                                    .foregroundStyle(server.connected ? theme.colors.secondary : theme.colors.primary)
                                    .font(.system(size: 12))

                                VStack(alignment: .leading) {
                                    Text(server.name)
                                        .font(.system(size: 13, weight: .medium))
                                    Text("\(server.command) • \(server.toolCount) tools")
                                        .font(.system(size: 10))
                                        .foregroundStyle(theme.colors.textMuted)
                                }

                                if server.error != nil {
                                    Image(systemName: "exclamationmark.triangle.fill")
                                        .foregroundStyle(.orange)
                                        .font(.system(size: 10))
                                }
                            }
                            .tag(server.id)
                        }
                    }
                    .listStyle(.bordered)
                }
            }
            .frame(minWidth: 220, idealWidth: 280)
            .padding(12)

            // Right pane
            if isAdding {
                addServerForm
            } else if let server = selectedServer {
                serverDetail(server)
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
                    TextField("e.g. filesystem", text: $newName)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Command")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    TextField("e.g. npx or /usr/local/bin/...", text: $newCommand)
                        .textFieldStyle(.roundedBorder)
                }

                VStack(alignment: .leading, spacing: 4) {
                    Text("Arguments")
                        .font(.system(size: 12, weight: .medium))
                        .foregroundStyle(theme.colors.text)
                    TextField("e.g. -y @modelcontextprotocol/...", text: $newArgs)
                        .textFieldStyle(.roundedBorder)
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
                    TextField("e.g. update_event, delete_event", text: $newExcluded)
                        .textFieldStyle(.roundedBorder)
                    Text("Tool names to hide from BoBe (server still exposes them)")
                        .font(.system(size: 10))
                        .foregroundStyle(theme.colors.textMuted)
                }

                HStack {
                    Button("Cancel") {
                        isAdding = false
                        clearForm()
                    }
                    .buttonStyle(.bordered)

                    Spacer()

                    Button("Add & Connect") { addServer() }
                        .buttonStyle(.borderedProminent)
                        .tint(theme.colors.primary)
                        .disabled(newName.isEmpty || newCommand.isEmpty)
                }

                if let error {
                    Text(error).font(.caption).foregroundStyle(.red)
                }
            }
            .padding(16)
        }
    }

    private func serverDetail(_ server: MCPServer) -> some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack(spacing: 8) {
                Text(server.name)
                    .font(.headline)
                    .foregroundStyle(theme.colors.text)

                Text(server.connected ? "connected" : "disconnected")
                    .font(.system(size: 10, weight: .medium))
                    .foregroundStyle(server.connected ? theme.colors.secondary : theme.colors.primary)
                    .padding(.horizontal, 8)
                    .padding(.vertical, 3)
                    .background(
                        Capsule().fill(
                            (server.connected ? theme.colors.secondary : theme.colors.primary).opacity(0.15)
                        )
                    )

                Text("\(server.toolCount) tools")
                    .font(.system(size: 10))
                    .foregroundStyle(theme.colors.textMuted)

                Spacer()
            }

            if let serverError = server.error {
                HStack(spacing: 6) {
                    Image(systemName: "exclamationmark.circle.fill")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.primary)
                    Text(serverError)
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.primary)
                }
            }

            // Excluded tools
            VStack(alignment: .leading, spacing: 6) {
                HStack(spacing: 6) {
                    Image(systemName: "nosign")
                        .font(.system(size: 12))
                    Text("Excluded Tools")
                        .font(.system(size: 13, weight: .semibold))
                    Text("(hidden from BoBe)")
                        .font(.system(size: 10))
                        .foregroundStyle(theme.colors.textMuted)
                }

                if server.excludedTools.isEmpty {
                    Text("No tools excluded")
                        .font(.system(size: 11))
                        .foregroundStyle(theme.colors.textMuted)
                } else {
                    FlowLayout(spacing: 4) {
                        ForEach(server.excludedTools, id: \.self) { tool in
                            HStack(spacing: 3) {
                                Text(tool)
                                    .font(.system(size: 10, design: .monospaced))
                                Button {
                                    removeExcludedTool(server: server, tool: tool)
                                } label: {
                                    Image(systemName: "xmark.circle.fill")
                                        .font(.system(size: 8))
                                }
                                .buttonStyle(.plain)
                            }
                            .padding(.horizontal, 6)
                            .padding(.vertical, 3)
                            .background(Capsule().fill(theme.colors.surface))
                        }
                    }
                }

                HStack(spacing: 6) {
                    TextField("tool name", text: $addExcludedText)
                        .textFieldStyle(.roundedBorder)
                        .frame(width: 150)
                        .onSubmit { addExcludedTool(server: server) }
                    Button("Add") { addExcludedTool(server: server) }
                        .buttonStyle(.bordered)
                        .controlSize(.small)
                        .disabled(addExcludedText.isEmpty)
                }
            }

            Divider()

            // Config display
            Text("Configuration")
                .font(.system(size: 13, weight: .semibold))
            Text("Command: \(server.command) \(server.args.joined(separator: " "))")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.colors.textMuted)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 6).fill(theme.colors.surface))

            Spacer()

            HStack(spacing: 8) {
                if deleteConfirm {
                    HStack(spacing: 6) {
                        Text("Remove server?")
                            .font(.system(size: 12))
                            .foregroundStyle(.red)
                        Button("Yes") { removeServer(server); deleteConfirm = false }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                            .tint(.red)
                        Button("No") { deleteConfirm = false }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                    }
                } else {
                    Button { deleteConfirm = true } label: {
                        Image(systemName: "trash")
                    }
                    .buttonStyle(.bordered)
                    .controlSize(.small)
                    .tint(.red)
                }

                Spacer()

                Button {
                    reconnectServer(server)
                } label: {
                    HStack(spacing: 4) {
                        if isReconnecting {
                            ProgressView().controlSize(.mini)
                        }
                        Text(isReconnecting ? "Reconnecting..." : "Reconnect")
                    }
                }
                .buttonStyle(.bordered)
                .controlSize(.small)
                .disabled(isReconnecting)
            }

            if let error {
                Text(error).font(.caption).foregroundStyle(.red)
            }
        }
        .padding(12)
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
