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
            self.headerSection
            self.errorSection
            self.excludedToolsSection
            Divider()
            self.configSection
            Spacer()
            self.actionsSection

            if let error {
                Text(error).font(.caption).foregroundStyle(self.theme.colors.primary)
            }
        }
        .frame(maxHeight: .infinity, alignment: .top)
        .padding(.horizontal, 12)
        .padding(.top, 12)
    }

    private var headerSection: some View {
        HStack(spacing: 8) {
            Text(self.server.name)
                .font(.headline)
                .foregroundStyle(self.theme.colors.text)

            Text(self.server.connected ? "connected" : "disconnected")
                .font(.system(size: 10, weight: .medium))
                .foregroundStyle(self.server.connected ? self.theme.colors.secondary : self.theme.colors.primary)
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(
                    Capsule().fill(
                        (self.server.connected ? self.theme.colors.secondary : self.theme.colors.primary).opacity(0.15)
                    )
                )

            Text("\(self.server.toolCount) tools")
                .font(.system(size: 10))
                .foregroundStyle(self.theme.colors.textMuted)

            Spacer()
        }
    }

    @ViewBuilder
    private var errorSection: some View {
        if let serverError = server.error {
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.circle.fill")
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.primary)
                Text(serverError)
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.primary)
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
                    .foregroundStyle(self.theme.colors.textMuted)
            }

            if self.server.excludedTools.isEmpty {
                Text("No tools excluded")
                    .font(.system(size: 11))
                    .foregroundStyle(self.theme.colors.textMuted)
            } else {
                FlowLayout(spacing: 4) {
                    ForEach(self.server.excludedTools, id: \.self) { tool in
                        HStack(spacing: 3) {
                            Text(tool)
                                .font(.system(size: 10, design: .monospaced))
                            Button {
                                self.onRemoveExcluded(self.server, tool)
                            } label: {
                                Image(systemName: "xmark.circle.fill")
                                    .font(.system(size: 8))
                            }
                            .bobeButton(.ghost, size: .mini)
                        }
                        .padding(.horizontal, 6)
                        .padding(.vertical, 3)
                        .background(Capsule().fill(self.theme.colors.surface))
                    }
                }
            }

            HStack(spacing: 6) {
                BobeTextField(placeholder: "tool name", text: self.$addExcludedText, width: 150) {
                    self.onAddExcluded(self.server)
                }
                Button("Add") { self.onAddExcluded(self.server) }
                    .bobeButton(.secondary, size: .small)
                    .disabled(self.addExcludedText.isEmpty)
            }
        }
    }

    private var configSection: some View {
        VStack(alignment: .leading, spacing: 4) {
            Text("Configuration")
                .font(.system(size: 13, weight: .semibold))
            Text("Command: \(self.server.command) \(self.server.args.joined(separator: " "))")
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(self.theme.colors.textMuted)
                .padding(8)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 6).fill(self.theme.colors.surface))
        }
    }

    private var actionsSection: some View {
        HStack(spacing: 8) {
            if self.deleteConfirm {
                HStack(spacing: 6) {
                    Text("Remove server?")
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.primary)
                    Button("Yes") {
                        self.onRemove(self.server)
                        self.deleteConfirm = false
                    }
                    .bobeButton(.destructive, size: .small)
                    Button("No") { self.deleteConfirm = false }
                        .bobeButton(.secondary, size: .small)
                }
            } else {
                Button {
                    self.deleteConfirm = true
                } label: {
                    Image(systemName: "trash")
                }
                .bobeButton(.destructive, size: .small)
            }

            Spacer()

            Button {
                self.onReconnect(self.server)
            } label: {
                HStack(spacing: 4) {
                    if self.isReconnecting {
                        BobeSpinner(size: 12)
                    }
                    Text(self.isReconnecting ? "Reconnecting..." : "Reconnect")
                }
            }
            .bobeButton(.secondary, size: .small)
            .disabled(self.isReconnecting)
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
