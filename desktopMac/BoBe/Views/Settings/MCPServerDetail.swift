import SwiftUI

/// Detail view for a single MCP server — connection status, excluded tools, config, actions.
struct MCPServerDetail: View {
    let server: MCPServer
    @Binding var deleteConfirm: Bool
    @Binding var isReconnecting: Bool
    @Binding var addExcludedText: String
    @Binding var error: String?
    var onReconnect: (MCPServer) -> Void
    var onRemove: (MCPServer) -> Void
    var onAddExcluded: (MCPServer) -> Void
    var onRemoveExcluded: (MCPServer, String) -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            headerSection
            errorSection
            excludedToolsSection
            Divider()
            configSection
            Spacer()
            actionsSection

            if let error {
                Text(error).font(.caption).foregroundStyle(theme.colors.primary)
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .padding(.horizontal, 12)
        .padding(.top, 12)
    }

    private var headerSection: some View {
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
    }

    @ViewBuilder
    private var errorSection: some View {
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
    }

    private var excludedToolsSection: some View {
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
                                onRemoveExcluded(server, tool)
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                                    .font(.system(size: 8))
                            }
                            .bobeButton(.ghost, size: .mini)
                        }
                        .padding(.horizontal, 6)
                        .padding(.vertical, 3)
                        .background(Capsule().fill(theme.colors.surface))
                    }
                }
            }

            HStack(spacing: 6) {
                BobeTextField(placeholder: "tool name", text: $addExcludedText, width: 150) {
                    onAddExcluded(server)
                }
                Button("Add") { onAddExcluded(server) }
                    .bobeButton(.secondary, size: .small)
                    .disabled(addExcludedText.isEmpty)
            }
        }
    }

    private var configSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Configuration")
                .font(.system(size: 13, weight: .semibold))
            Text("Command: \(server.command) \(server.args.joined(separator: " "))")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(theme.colors.textMuted)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 6).fill(theme.colors.surface))
        }
    }

    private var actionsSection: some View {
        HStack(spacing: 8) {
            if deleteConfirm {
                HStack(spacing: 6) {
                    Text("Remove server?")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.primary)
                    Button("Yes") { onRemove(server); deleteConfirm = false }
                        .bobeButton(.destructive, size: .small)
                    Button("No") { deleteConfirm = false }
                        .bobeButton(.secondary, size: .small)
                }
            } else {
                Button { deleteConfirm = true } label: {
                    Image(systemName: "trash")
                }
                .bobeButton(.destructive, size: .small)
            }

            Spacer()

            Button {
                onReconnect(server)
            } label: {
                HStack(spacing: 4) {
                    if isReconnecting {
                        BobeSpinner(size: 12)
                    }
                    Text(isReconnecting ? "Reconnecting..." : "Reconnect")
                }
            }
            .bobeButton(.secondary, size: .small)
            .disabled(isReconnecting)
        }
    }
}

// MARK: - Previews

#Preview("MCP Server Detail") {
    @Previewable @State var deleteConfirm = false
    @Previewable @State var isReconnecting = false
    @Previewable @State var addExcludedText = ""
    @Previewable @State var error: String? = nil
    MCPServerDetail(
        server: MCPServer(
            id: "preview-1",
            name: "filesystem",
            command: "npx",
            args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
            connected: true,
            enabled: true,
            toolCount: 5,
            excludedTools: ["delete_file"]
        ),
        deleteConfirm: $deleteConfirm,
        isReconnecting: $isReconnecting,
        addExcludedText: $addExcludedText,
        error: $error,
        onReconnect: { _ in },
        onRemove: { _ in },
        onAddExcluded: { _ in },
        onRemoveExcluded: { _, _ in }
    )
    .environment(\.theme, allThemes[0])
    .frame(width: 400, height: 500)
}
