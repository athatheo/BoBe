import SwiftUI

/// Tools list panel — grouped by provider with expandable descriptions and BobeToggle.
/// Based on ToolsSettings.tsx with provider groups, stats, and expand/collapse.
struct ToolsPanel: View {
    @State private var tools: [ToolInfo] = []
    @State private var isLoading = false
    @State private var error: String?
    @State private var expandedTool: String?
    @Environment(\.theme) private var theme

    private var enabledCount: Int { tools.filter(\.enabled).count }
    private var providers: [String] { Array(Set(tools.map(\.provider))).sorted() }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 16) {
                HStack {
                    Text("Tools")
                        .font(.title2.bold())
                        .foregroundStyle(theme.colors.text)
                    Spacer()

                    Text("\(enabledCount) of \(tools.count) enabled")
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.textMuted)

                    Button { Task { await loadTools() } } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .bobeButton(.secondary, size: .small)
                }

                if let error {
                    HStack(spacing: 6) {
                        Image(systemName: "exclamationmark.triangle.fill")
                            .foregroundStyle(theme.colors.primary)
                        Text(error)
                            .font(.system(size: 12))
                            .foregroundStyle(theme.colors.primary)
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .background(RoundedRectangle(cornerRadius: 8).fill(theme.colors.primary.opacity(0.08)))
                }

                if isLoading && tools.isEmpty {
                    HStack(spacing: 8) {
                        BobeSpinner(size: 14)
                        Text("Loading tools...")
                            .font(.system(size: 13))
                            .foregroundStyle(theme.colors.textMuted)
                    }
                    .frame(maxWidth: .infinity, alignment: .center)
                    .padding(.top, 40)
                } else if tools.isEmpty && !isLoading {
                    VStack(spacing: 8) {
                        Image(systemName: "wrench.and.screwdriver")
                            .font(.system(size: 28))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("No tools available")
                            .font(.system(size: 13, weight: .medium))
                            .foregroundStyle(theme.colors.textMuted)
                        Text("Tools become available when the daemon is running")
                            .font(.system(size: 11))
                            .foregroundStyle(theme.colors.textMuted.opacity(0.7))
                    }
                    .frame(maxWidth: .infinity)
                    .padding(.top, 40)
                } else {
                    ForEach(providers, id: \.self) { provider in
                        providerGroup(provider)
                    }
                }
            }
            .padding(24)
        }
        .task { await loadTools() }
    }

    private func providerGroup(_ provider: String) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            Text(provider.uppercased())
                .font(.system(size: 11, weight: .bold))
                .foregroundStyle(theme.colors.textMuted)
                .padding(.horizontal, 8)
                .padding(.vertical, 4)
                .frame(maxWidth: .infinity, alignment: .leading)
                .background(RoundedRectangle(cornerRadius: 6).fill(theme.colors.surface))

            ForEach(tools.filter { $0.provider == provider }) { tool in
                VStack(alignment: .leading, spacing: 0) {
                    HStack(spacing: 8) {
                        VStack(alignment: .leading, spacing: 2) {
                            HStack(spacing: 6) {
                                Text(tool.name)
                                    .font(.system(size: 13, weight: .medium))
                                    .foregroundStyle(theme.colors.text)

                                if let category = tool.category {
                                    Text(category)
                                        .font(.system(size: 9))
                                        .padding(.horizontal, 5)
                                        .padding(.vertical, 1)
                                        .background(Capsule().fill(theme.colors.border.opacity(0.5)))
                                        .foregroundStyle(theme.colors.textMuted)
                                }
                            }

                            if expandedTool != tool.name {
                                HStack(spacing: 4) {
                                    Text(tool.description.count > 80
                                         ? String(tool.description.prefix(80)) + "..."
                                         : tool.description)
                                        .font(.system(size: 11))
                                        .foregroundStyle(theme.colors.textMuted)
                                        .lineLimit(1)

                                    if tool.description.count > 80 {
                                        Image(systemName: "chevron.down")
                                            .font(.system(size: 8))
                                            .foregroundStyle(theme.colors.textMuted)
                                    }
                                }
                                .onTapGesture {
                                    if tool.description.count > 80 {
                                        withAnimation(.easeOut(duration: 0.15)) {
                                            expandedTool = tool.name
                                        }
                                    }
                                }
                            }
                        }

                        Spacer()

                        BobeToggle(isOn: Binding(
                            get: { tool.enabled },
                            set: { _ in toggleTool(tool) }
                        ))
                    }
                    .padding(.vertical, 6)
                    .padding(.horizontal, 4)

                    if expandedTool == tool.name {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(tool.description)
                                .font(.system(size: 12))
                                .foregroundStyle(theme.colors.textMuted)
                                .fixedSize(horizontal: false, vertical: true)
                                .padding(10)
                                .frame(maxWidth: .infinity, alignment: .leading)
                                .background(RoundedRectangle(cornerRadius: 6).fill(theme.colors.surface))

                            Button {
                                withAnimation(.easeOut(duration: 0.15)) {
                                    expandedTool = nil
                                }
                            } label: {
                                HStack(spacing: 4) {
                                    Text("collapse")
                                        .font(.system(size: 10))
                                    Image(systemName: "chevron.up")
                                        .font(.system(size: 8))
                                }
                                .foregroundStyle(theme.colors.textMuted)
                            }
                            .bobeButton(.ghost, size: .mini)
                        }
                        .padding(.horizontal, 4)
                        .padding(.bottom, 4)
                        .transition(.opacity.combined(with: .move(edge: .top)))
                    }
                }
            }
        }
    }

    private func loadTools() async {
        isLoading = true
        defer { isLoading = false }
        do {
            let resp = try await DaemonClient.shared.listTools()
            tools = resp.tools
            error = nil
        } catch { self.error = error.localizedDescription }
    }

    private func toggleTool(_ tool: ToolInfo) {
        Task {
            do {
                if tool.enabled {
                    _ = try await DaemonClient.shared.disableTool(tool.name)
                } else {
                    _ = try await DaemonClient.shared.enableTool(tool.name)
                }
                await loadTools()
            } catch { self.error = error.localizedDescription }
        }
    }
}
