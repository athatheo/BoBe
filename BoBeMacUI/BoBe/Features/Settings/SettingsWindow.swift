import SwiftUI

enum SettingsCategory: String, CaseIterable, Identifiable {
    case souls, goals, memories
    case userProfiles = "user-profiles"
    case mcpServers = "mcp-servers"
    case engine
    case voice
    case appearance, behavior, privacy
    case expert

    var id: String {
        rawValue
    }

    var label: String {
        switch self {
        case .souls: L10n.tr("settings.category.souls")
        case .goals: L10n.tr("settings.category.goals")
        case .memories: L10n.tr("settings.category.memories")
        case .userProfiles: L10n.tr("settings.category.user_profiles")
        case .mcpServers: L10n.tr("settings.category.mcp_servers")
        case .engine: L10n.tr("settings.category.engine")
        case .voice: L10n.tr("settings.category.voice")
        case .appearance: L10n.tr("settings.category.appearance")
        case .behavior: L10n.tr("settings.category.behavior")
        case .privacy: L10n.tr("settings.category.privacy")
        case .expert: L10n.tr("settings.category.expert")
        }
    }

    var icon: String {
        switch self {
        case .souls: "sparkles"
        case .goals: "target"
        case .memories: "brain.head.profile"
        case .userProfiles: "person.fill"
        case .mcpServers: "server.rack"
        case .engine: "cpu"
        case .voice: "waveform"
        case .appearance: "paintpalette.fill"
        case .behavior: "slider.horizontal.3"
        case .privacy: "shield.fill"
        case .expert: "wrench.and.screwdriver.fill"
        }
    }

    /// Categories hidden until Expert mode is on. None today — Expert
    /// itself lives under PREFERENCES so it's always reachable. The
    /// former Advanced panel was folded into the panels its toggles
    /// actually belonged to (MCP, Behavior).
    var requiresExpertMode: Bool {
        false
    }
}

enum SettingsCategoryGroup: String, CaseIterable {
    case context = "CONTEXT"
    case integrations = "INTEGRATIONS"
    case preferences = "PREFERENCES"

    var categories: [SettingsCategory] {
        switch self {
        case .context:
            [.souls, .goals, .memories, .userProfiles]
        case .integrations:
            [.mcpServers, .engine]
        case .preferences:
            [.voice, .appearance, .behavior, .privacy, .expert]
        }
    }

    var label: String {
        switch self {
        case .context: L10n.tr("settings.group.context")
        case .integrations: L10n.tr("settings.group.integrations")
        case .preferences: L10n.tr("settings.group.preferences")
        }
    }
}

struct SettingsWindow: View {
    /// Optional initial category to navigate to on first appearance. Used by
    /// the overlay's MicButton "needs setup" deep-link to land directly on
    /// the Voice pane, and the Finish-Setup pill to land on Engine/Voice.
    let initialCategory: SettingsCategory?

    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var selectedCategory: SettingsCategory?
    @State private var searchQuery = ""
    @State private var expertMode = ExpertMode.shared
    private let themeStore = ThemeStore.shared
    private let store = BobeStore.shared

    init(initialCategory: SettingsCategory? = nil) {
        self.initialCategory = initialCategory
        // SwiftUI ignores the initial value of @State after the first render,
        // so we set it explicitly via _selectedCategory's wrapped value.
        self._selectedCategory = State(initialValue: initialCategory)
    }

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
        .frame(minWidth: 760, minHeight: 520)
        .onChange(of: self.reduceMotion, initial: true) { _, new in
            OverlayMotionRuntime.reduceMotion = new
        }
        .id(self.store.localeOverride)
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
            .padding(.bottom, 8)
            .frame(maxWidth: .infinity)
            .frame(height: 44)
            .background(self.headerGradient)
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(self.theme.colors.border)
                    .frame(height: 1)
            }

            // Search field. Filters category visibility by case-insensitive
            // label substring. Pure client-side, no daemon round-trip.
            HStack(spacing: 6) {
                Image(systemName: "magnifyingglass")
                    .font(.system(size: 11, weight: .medium))
                    .foregroundStyle(self.theme.colors.textMuted)
                TextField(L10n.tr("settings.window.search.placeholder"), text: self.$searchQuery)
                    .textFieldStyle(.plain)
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.text)
                if !self.searchQuery.isEmpty {
                    Button {
                        self.searchQuery = ""
                    } label: {
                        Image(systemName: "xmark.circle.fill")
                            .font(.system(size: 11))
                            .foregroundStyle(self.theme.colors.textMuted)
                    }
                    .buttonStyle(.plain)
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 6)
                    .fill(self.theme.colors.surface)
                    .overlay(
                        RoundedRectangle(cornerRadius: 6)
                            .stroke(self.theme.colors.border.opacity(0.5), lineWidth: 1)
                    )
            )
            .padding(.horizontal, 12)
            .padding(.top, 8)
            .padding(.bottom, 4)

            List {
                ForEach(SettingsCategoryGroup.allCases, id: \.self) { group in
                    let visible = self.visibleCategories(in: group)
                    if !visible.isEmpty {
                        Section {
                            ForEach(visible) { category in
                                Button { self.selectedCategory = category } label: {
                                    self.sidebarRow(for: category)
                                }
                                .buttonStyle(.plain)
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
                if self.allVisibleCategories.isEmpty {
                    if !self.expertModeGatedMatches.isEmpty, !self.expertMode.isEnabled {
                        // The query matched at least one Expert-only category
                        // while Expert mode is off. Don't leave the user in a
                        // dead-end — explain it and offer the unlock.
                        VStack(alignment: .leading, spacing: 8) {
                            Text(L10n.tr("settings.window.search.expert_gated"))
                                .font(.system(size: 11))
                                .foregroundStyle(self.theme.colors.textMuted)
                                .fixedSize(horizontal: false, vertical: true)
                            Button(L10n.tr("settings.window.search.enable_expert")) {
                                self.expertMode.setEnabled(true)
                            }
                            .buttonStyle(.bordered)
                            .controlSize(.small)
                        }
                        .padding(.horizontal, 8)
                        .padding(.vertical, 12)
                        .listRowSeparator(.hidden)
                        .listRowBackground(Color.clear)
                    } else {
                        Text(String(format: L10n.tr("settings.window.search.no_results"), self.searchQuery))
                            .font(.system(size: 11))
                            .foregroundStyle(self.theme.colors.textMuted)
                            .padding(.horizontal, 8)
                            .padding(.vertical, 12)
                            .listRowSeparator(.hidden)
                            .listRowBackground(Color.clear)
                    }
                }
            }
            .listStyle(.sidebar)
            .tint(self.theme.colors.primary)
            .scrollContentBackground(.hidden)
            .padding(.top, 2)
        }
        .background(self.theme.colors.background)
    }

    private var allVisibleCategories: [SettingsCategory] {
        SettingsCategoryGroup.allCases.flatMap { self.visibleCategories(in: $0) }
    }

    /// Categories whose label matches the current query but that are hidden
    /// because Expert mode is off. Used to surface an "Enable Expert mode"
    /// affordance instead of an empty-state dead-end when the user is
    /// hunting for something like "Advanced" without the flag on.
    private var expertModeGatedMatches: [SettingsCategory] {
        let query = self.searchQuery.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard !query.isEmpty else { return [] }
        return SettingsCategory.allCases.filter { category in
            category.requiresExpertMode && category.label.lowercased().contains(query)
        }
    }

    private func visibleCategories(in group: SettingsCategoryGroup) -> [SettingsCategory] {
        let query = self.searchQuery.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return group.categories.filter { category in
            if category.requiresExpertMode, !self.expertMode.isEnabled {
                return false
            }
            guard !query.isEmpty else { return true }
            return category.label.lowercased().contains(query)
        }
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
        .padding(.bottom, 8)
        .frame(height: 44)
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
        case .mcpServers:
            MCPServersPanel()
        case .engine:
            EnginePanel()
        case .voice:
            VoicePanel()
        case .appearance:
            AppearancePanel()
        case .behavior:
            BehaviorPanel()
        case .privacy:
            PrivacyPanel()
        case .expert:
            ExpertModePanel()
        }
    }
}

struct SettingsOverview: View {
    var onNavigate: (SettingsCategory) -> Void
    @Environment(\.theme) private var theme

    @State private var goalsActive: Int?
    @State private var soulsCustom: Int?
    @State private var profileKnown: Bool?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 0) {
                Text(L10n.tr("settings.window.overview.eyebrow"))
                    .font(.system(size: 11, weight: .semibold))
                    .tracking(2.4)
                    .foregroundStyle(self.theme.colors.textMuted)
                    .padding(.top, 48)

                Text(L10n.tr("settings.window.overview.title"))
                    .font(.system(size: 30, weight: .bold))
                    .foregroundStyle(self.theme.colors.text)
                    .padding(.top, 10)

                Text(L10n.tr("settings.window.overview.lede"))
                    .font(.system(size: 14))
                    .foregroundStyle(self.theme.colors.textMuted)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.top, 14)
                    .frame(maxWidth: 540, alignment: .leading)

                VStack(spacing: 0) {
                    self.adaptiveRow(
                        label: self.goalsLabel,
                        actionLabel: L10n.tr("settings.window.overview.row.goals.action"),
                        accent: self.theme.colors.primary,
                        category: .goals
                    )
                    self.adaptiveRow(
                        label: self.soulsLabel,
                        actionLabel: L10n.tr("settings.window.overview.row.souls.action"),
                        accent: self.theme.colors.secondary,
                        category: .souls
                    )
                    self.adaptiveRow(
                        label: self.profilesLabel,
                        actionLabel: L10n.tr("settings.window.overview.row.profiles.action"),
                        accent: self.theme.colors.tertiary,
                        category: .userProfiles
                    )
                    self.adaptiveRow(
                        label: L10n.tr("settings.window.overview.row.behavior.label"),
                        actionLabel: L10n.tr("settings.window.overview.row.behavior.action"),
                        accent: self.theme.colors.primary.opacity(0.7),
                        category: .behavior,
                        isLast: true
                    )
                }
                .padding(.top, 32)

                Text(L10n.tr("settings.window.overview.footer"))
                    .font(.system(size: 12))
                    .foregroundStyle(self.theme.colors.textMuted)
                    .padding(.top, 28)
                    .padding(.bottom, 48)
            }
            .frame(maxWidth: 620, alignment: .leading)
            .padding(.horizontal, 40)
            .frame(maxWidth: .infinity, alignment: .leading)
        }
        .task { await self.loadCounts() }
    }

    // MARK: - Adaptive copy

    private var goalsLabel: String {
        switch self.goalsActive {
        case .none, 0:
            L10n.tr("settings.window.overview.row.goals.empty")
        case 1:
            L10n.tr("settings.window.overview.row.goals.known_one")
        case let count?:
            L10n.tr("settings.window.overview.row.goals.known_many_format", count)
        }
    }

    private var soulsLabel: String {
        switch self.soulsCustom {
        case .none, 0:
            L10n.tr("settings.window.overview.row.souls.empty")
        case 1:
            L10n.tr("settings.window.overview.row.souls.known_one")
        case let count?:
            L10n.tr("settings.window.overview.row.souls.known_many_format", count)
        }
    }

    private var profilesLabel: String {
        switch self.profileKnown {
        case .some(true):
            L10n.tr("settings.window.overview.row.profiles.known")
        default:
            L10n.tr("settings.window.overview.row.profiles.empty")
        }
    }

    // MARK: - Row

    private func adaptiveRow(
        label: String,
        actionLabel: String,
        accent: Color,
        category: SettingsCategory,
        isLast: Bool = false
    ) -> some View {
        AdaptiveOverviewRow(
            label: label,
            actionLabel: actionLabel,
            accent: accent,
            category: category,
            isLast: isLast,
            onNavigate: self.onNavigate
        )
    }

    private func loadCounts() async {
        async let goalsTask = try? await DaemonClient.shared.listGoals()
        async let soulsTask = try? await DaemonClient.shared.listSouls()
        async let profilesTask = try? await DaemonClient.shared.listUserProfiles()

        let goals = await goalsTask
        let souls = await soulsTask
        let profiles = await profilesTask

        self.goalsActive = goals?.activeCount
        // SoulListResponse.count includes the default soul; only show "custom" personalities.
        self.soulsCustom = souls.map { max(0, $0.count - 1) }
        self.profileKnown = profiles.map { list in list.profiles.contains { !$0.isDefault } }
    }
}

private struct AdaptiveOverviewRow: View {
    let label: String
    let actionLabel: String
    let accent: Color
    let category: SettingsCategory
    let isLast: Bool
    let onNavigate: (SettingsCategory) -> Void

    @Environment(\.theme) private var theme
    @State private var isHovered = false

    var body: some View {
        Button { self.onNavigate(self.category) } label: {
            HStack(alignment: .firstTextBaseline, spacing: 16) {
                Text(self.label)
                    .font(.system(size: 16, weight: .regular))
                    .foregroundStyle(self.theme.colors.text)
                    .fixedSize(horizontal: false, vertical: true)
                    .frame(maxWidth: .infinity, alignment: .leading)

                HStack(spacing: 6) {
                    Text(self.actionLabel)
                        .font(.system(size: 13, weight: .semibold))
                    Image(systemName: "arrow.right")
                        .font(.system(size: 11, weight: .semibold))
                        .offset(x: self.isHovered ? 3 : 0)
                }
                .foregroundStyle(self.accent)
                .opacity(self.isHovered ? 1 : 0.78)
            }
            .padding(.vertical, 18)
            .overlay(alignment: .bottom) {
                if !self.isLast {
                    Rectangle()
                        .fill(self.theme.colors.border.opacity(self.isHovered ? 0.9 : 0.5))
                        .frame(height: 1)
                }
            }
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .onHover { hover in
            withAnimation(.easeOut(duration: 0.18)) { self.isHovered = hover }
        }
    }
}
