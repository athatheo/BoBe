import SwiftUI

/// Settings category for sidebar navigation
enum SettingsCategory: String, CaseIterable, Identifiable {
    case souls, goals, memories
    case userProfiles = "user-profiles"
    case tools
    case mcpServers = "mcp-servers"
    case appearance
    case aiModel = "ai-model"
    case behavior, privacy
    case goalWorker = "goal-worker"
    case advanced

    var id: String {
        rawValue
    }

    var label: String {
        switch self {
        case .souls: "Souls"
        case .goals: "Goals"
        case .memories: "Memories"
        case .userProfiles: "User Profiles"
        case .tools: "Tools"
        case .mcpServers: "MCP Servers"
        case .appearance: "Appearance"
        case .aiModel: "AI Model"
        case .behavior: "Behavior"
        case .privacy: "Privacy"
        case .goalWorker: "Goal Worker"
        case .advanced: "For Nerds"
        }
    }

    var icon: String {
        switch self {
        case .souls: "sparkles"
        case .goals: "target"
        case .memories: "brain.head.profile"
        case .userProfiles: "person.fill"
        case .tools: "wrench.fill"
        case .mcpServers: "server.rack"
        case .appearance: "paintpalette.fill"
        case .aiModel: "cpu.fill"
        case .behavior: "slider.horizontal.3"
        case .privacy: "shield.fill"
        case .goalWorker: "gearshape.2.fill"
        case .advanced: "terminal.fill"
        }
    }

}

enum SettingsCategoryGroup: String, CaseIterable {
    case context = "CONTEXT"
    case integrations = "INTEGRATIONS"
    case preferences = "PREFERENCES"
    case advanced = "ADVANCED"

    var categories: [SettingsCategory] {
        switch self {
        case .context:
            [.souls, .goals, .memories, .userProfiles]
        case .integrations:
            [.tools, .mcpServers]
        case .preferences:
            [.appearance, .aiModel, .behavior, .privacy, .goalWorker]
        case .advanced:
            [.advanced]
        }
    }
}

/// Main settings window view with sidebar + content
struct SettingsWindow: View {
    @State private var selectedCategory: SettingsCategory?
    @State private var themeStore = ThemeStore.shared

    private var theme: ThemeConfig {
        self.themeStore.currentTheme
    }

    private var headerGradient: LinearGradient {
        LinearGradient(
            colors: [self.theme.colors.tertiary.opacity(0.25), self.theme.colors.border.opacity(0.2)],
            startPoint: .top,
            endPoint: .bottom
        )
    }

    var body: some View {
        NavigationSplitView {
            self.settingsSidebar
                .navigationSplitViewColumnWidth(min: 220, ideal: 220, max: 280)
        } detail: {
            VStack(spacing: 0) {
                self.settingsHeader
                self.settingsContent
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .background(self.theme.colors.background)
        }
        .navigationSplitViewStyle(.balanced)
        .environment(\.theme, self.theme)
        .preferredColorScheme(self.theme.isDark ? .dark : .light)
        .background(self.theme.colors.background)
        .ignoresSafeArea(.container, edges: .top)
        .toolbar(removing: .sidebarToggle)
    }

    // MARK: - Sidebar

    private var settingsSidebar: some View {
        VStack(spacing: 0) {
            HStack {
                Text("BOBE TUNING")
                    .bobeTextStyle(.sectionLabel)
                    .tracking(1.2)
                    .foregroundStyle(self.theme.colors.primary)
                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 12)
            .frame(maxWidth: .infinity)
            .frame(height: 80)
            .background(self.headerGradient)
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(self.theme.colors.border)
                    .frame(height: 1)
            }

            List {
                ForEach(SettingsCategoryGroup.allCases, id: \.self) { group in
                    Section {
                        ForEach(group.categories) { category in
                            self.sidebarRow(for: category)
                                .onTapGesture { self.selectedCategory = category }
                                .listRowInsets(.init(top: 2, leading: 8, bottom: 2, trailing: 8))
                                .listRowSeparator(.hidden)
                                .listRowBackground(Color.clear)
                        }
                    } header: {
                        Text(group.rawValue)
                            .bobeTextStyle(.sectionLabel)
                            .tracking(0.8)
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                }
            }
            .listStyle(.sidebar)
            .tint(self.theme.colors.primary)
            .scrollContentBackground(.hidden)
        }
        .background(self.theme.colors.background)
    }

    private func sidebarRow(for category: SettingsCategory) -> some View {
        let isSelected = self.selectedCategory == category

        return HStack(spacing: 10) {
            Image(systemName: category.icon)
                .font(.system(size: 13, weight: .semibold))
                .foregroundStyle(isSelected ? self.theme.colors.primary : self.theme.colors.textMuted)
                .frame(width: 16)

            Text(category.label)
                .font(.system(size: 13, weight: isSelected ? .semibold : .regular))
                .foregroundStyle(self.theme.colors.text)

            Spacer(minLength: 0)
        }
        .padding(.horizontal, 8)
        .padding(.vertical, 7)
        .background(
            RoundedRectangle(cornerRadius: 8)
                .fill(isSelected ? self.theme.colors.primary.opacity(self.theme.isDark ? 0.24 : 0.14) : .clear)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 8)
                .stroke(isSelected ? self.theme.colors.primary.opacity(0.55) : .clear, lineWidth: 1)
        )
        .contentShape(Rectangle())
    }

    private var settingsHeader: some View {
        HStack(alignment: .bottom) {
            Text(self.selectedCategory?.label ?? "")
                .bobeTextStyle(.windowTitle)
                .foregroundStyle(self.theme.colors.text)
            Spacer()
            Button("Check for Updates...") {
                UpdaterManager.shared.checkForUpdates()
            }
            .buttonStyle(.bordered)
            .disabled(!UpdaterManager.shared.canCheckForUpdates)
        }
        .padding(.horizontal, 24)
        .padding(.bottom, 12)
        .frame(height: 80)
        .background(self.headerGradient)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(self.theme.colors.border)
                .frame(height: 1)
        }
    }

    // MARK: - Content

    @ViewBuilder
    private var settingsContent: some View {
        switch self.selectedCategory {
        case nil:
            SettingsOverview(onNavigate: { self.selectedCategory = $0 })
        case .souls:
            SoulsEditor()
        case .goals:
            GoalsEditor()
        case .memories:
            MemoriesEditor()
        case .userProfiles:
            UserProfilesEditor()
        case .tools:
            ToolsPanel()
        case .mcpServers:
            MCPServersPanel()
        case .appearance:
            AppearancePanel()
        case .aiModel:
            AIModelPanel()
        case .behavior:
            BehaviorPanel()
        case .privacy:
            PrivacyPanel()
        case .goalWorker:
            GoalWorkerPanel()
        case .advanced:
            AdvancedPanel()
        }
    }
}

// MARK: - Settings Overview Page

struct SettingsOverview: View {
    var onNavigate: (SettingsCategory) -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                VStack(spacing: 8) {
                    Text("How to change BoBe")
                        .font(.system(size: 24, weight: .bold))
                        .foregroundStyle(self.theme.colors.text)

                    Text(
                        "BoBe uses a personality (Soul), your goals, and memories to provide contextual, "
                            + "proactive assistance. Here's how to customize your experience."
                    )
                    .font(.system(size: 14))
                    .foregroundStyle(self.theme.colors.textMuted)
                    .multilineTextAlignment(.center)
                    .frame(maxWidth: 480)
                }
                .padding(.top, 32)

                LazyVGrid(
                    columns: [
                        GridItem(.flexible(), spacing: 16),
                        GridItem(.flexible(), spacing: 16),
                    ], spacing: 16
                ) {
                    self.overviewCard(
                        icon: "eye.fill",
                        color: self.theme.colors.primary,
                        heading: "What BoBe sees",
                        body: "Control screen capture, context window, and what information BoBe has access to.",
                        target: .behavior
                    )
                    self.overviewCard(
                        icon: "brain.head.profile",
                        color: self.theme.colors.secondary,
                        heading: "What BoBe remembers",
                        body: "Manage memories, conversation history, and how BoBe learns from interactions.",
                        target: .memories
                    )
                    self.overviewCard(
                        icon: "message.fill",
                        color: self.theme.colors.tertiary,
                        heading: "When BoBe speaks up",
                        body: "Configure check-in frequency, proactive messages, and notification preferences.",
                        target: .behavior
                    )
                    self.overviewCard(
                        icon: "paintbrush.fill",
                        color: self.theme.colors.primary.opacity(0.7),
                        heading: "How BoBe sounds",
                        body: "Choose and customize personality templates that shape BoBe's communication style.",
                        target: .souls
                    )
                    self.overviewCard(
                        icon: "bolt.fill",
                        color: self.theme.colors.secondary,
                        heading: "What BoBe can do",
                        body: "Enable tools, connect MCP servers, and extend BoBe's capabilities.",
                        target: .tools
                    )
                }
                .padding(.horizontal, 24)

                Text("Use the sidebar to explore all settings. Everything runs locally on your Mac.")
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)
                    .padding(.bottom, 24)
            }
        }
    }

    private func overviewCard(
        icon: String,
        color: Color,
        heading: String,
        body: String,
        target: SettingsCategory
    ) -> some View {
        Button {
            self.onNavigate(target)
        } label: {
            HStack(spacing: 14) {
                Image(systemName: icon)
                    .font(.system(size: 22))
                    .foregroundStyle(color)
                    .frame(width: 36, height: 36)

                VStack(alignment: .leading, spacing: 4) {
                    Text(heading)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(self.theme.colors.text)

                    Text(body)
                        .font(.system(size: 12))
                        .foregroundStyle(self.theme.colors.textMuted)
                        .lineLimit(3)
                        .multilineTextAlignment(.leading)
                }

                Spacer()

                Image(systemName: "chevron.right")
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .padding(16)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(self.theme.colors.background)
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(self.theme.colors.border, lineWidth: 1)
                    )
                    .shadow(color: Color.black.opacity(0.06), radius: 4, y: 2)
            )
        }
        .buttonStyle(.plain)
    }
}
