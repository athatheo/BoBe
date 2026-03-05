import SwiftUI

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
        case .souls: L10n.tr("settings.category.souls")
        case .goals: L10n.tr("settings.category.goals")
        case .memories: L10n.tr("settings.category.memories")
        case .userProfiles: L10n.tr("settings.category.user_profiles")
        case .tools: L10n.tr("settings.category.tools")
        case .mcpServers: L10n.tr("settings.category.mcp_servers")
        case .appearance: L10n.tr("settings.category.appearance")
        case .aiModel: L10n.tr("settings.category.ai_model")
        case .behavior: L10n.tr("settings.category.behavior")
        case .privacy: L10n.tr("settings.category.privacy")
        case .goalWorker: L10n.tr("settings.category.goal_worker")
        case .advanced: L10n.tr("settings.category.advanced")
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

    var label: String {
        switch self {
        case .context: L10n.tr("settings.group.context")
        case .integrations: L10n.tr("settings.group.integrations")
        case .preferences: L10n.tr("settings.group.preferences")
        case .advanced: L10n.tr("settings.group.advanced")
        }
    }
}

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

    private var settingsSidebar: some View {
        VStack(spacing: 0) {
            HStack {
                Text(L10n.tr("settings.window.sidebar_title"))
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
                        Text(group.label)
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
            Button(L10n.tr("settings.window.check_updates")) {
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

struct SettingsOverview: View {
    var onNavigate: (SettingsCategory) -> Void
    @Environment(\.theme) private var theme

    var body: some View {
        ScrollView {
            VStack(spacing: 24) {
                VStack(spacing: 8) {
                    Text(L10n.tr("settings.window.overview.title"))
                        .font(.system(size: 24, weight: .bold))
                        .foregroundStyle(self.theme.colors.text)

                    Text(L10n.tr("settings.window.overview.description"))
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
                        heading: L10n.tr("settings.window.overview.card.sees.heading"),
                        body: L10n.tr("settings.window.overview.card.sees.body"),
                        target: .behavior
                    )
                    self.overviewCard(
                        icon: "brain.head.profile",
                        color: self.theme.colors.secondary,
                        heading: L10n.tr("settings.window.overview.card.remembers.heading"),
                        body: L10n.tr("settings.window.overview.card.remembers.body"),
                        target: .memories
                    )
                    self.overviewCard(
                        icon: "message.fill",
                        color: self.theme.colors.tertiary,
                        heading: L10n.tr("settings.window.overview.card.speaks.heading"),
                        body: L10n.tr("settings.window.overview.card.speaks.body"),
                        target: .behavior
                    )
                    self.overviewCard(
                        icon: "paintbrush.fill",
                        color: self.theme.colors.primary.opacity(0.7),
                        heading: L10n.tr("settings.window.overview.card.sounds.heading"),
                        body: L10n.tr("settings.window.overview.card.sounds.body"),
                        target: .souls
                    )
                    self.overviewCard(
                        icon: "bolt.fill",
                        color: self.theme.colors.secondary,
                        heading: L10n.tr("settings.window.overview.card.can_do.heading"),
                        body: L10n.tr("settings.window.overview.card.can_do.body"),
                        target: .tools
                    )
                }
                .padding(.horizontal, 24)

                Text(L10n.tr("settings.window.overview.footer"))
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
