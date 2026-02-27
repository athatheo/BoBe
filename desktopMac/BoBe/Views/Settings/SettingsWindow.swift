import SwiftUI

/// Settings category for sidebar navigation
enum SettingsCategory: String, CaseIterable, Identifiable {
    case souls, goals, memories, userProfiles = "user-profiles"
    case tools, mcpServers = "mcp-servers"
    case appearance, aiModel = "ai-model", behavior, privacy
    case advanced

    var id: String { rawValue }

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
        case .advanced: "terminal.fill"
        }
    }

    var group: SettingsCategoryGroup? {
        switch self {
        case .souls, .goals, .memories, .userProfiles: .context
        case .tools, .mcpServers: .integrations
        case .appearance, .aiModel, .behavior, .privacy: .preferences
        case .advanced: .advanced
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
            [.appearance, .aiModel, .behavior, .privacy]
        case .advanced:
            [.advanced]
        }
    }
}

/// Main settings window view with sidebar + content
struct SettingsWindow: View {
    @State private var selectedCategory: SettingsCategory? = nil
    @State private var themeStore = ThemeStore.shared

    private var theme: ThemeConfig { themeStore.currentTheme }
    private var headerGradient: LinearGradient {
        LinearGradient(
            colors: [theme.colors.tertiary.opacity(0.25), theme.colors.border.opacity(0.2)],
            startPoint: .top,
            endPoint: .bottom
        )
    }

    var body: some View {
        HStack(spacing: 0) {
            settingsSidebar
                .frame(width: 220)

            VStack(spacing: 0) {
                settingsHeader
                settingsContent
            }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(theme.colors.background)
        }
        .environment(\.theme, theme)
        .preferredColorScheme(theme.isDark ? .dark : .light)
        .background(theme.colors.background)
    }

    // MARK: - Sidebar (matches settings-sidebar CSS)

    private var settingsSidebar: some View {
        VStack(spacing: 0) {
            // Header (80px with traffic-light clearance)
            HStack {
                Text("BOBE TUNING")
                    .font(.system(size: 11, weight: .bold))
                    .tracking(1.2)
                    .foregroundStyle(theme.colors.primary)
                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.bottom, 12)
            .frame(maxWidth: .infinity)
            .frame(height: 80)
            .background(headerGradient)
            .overlay(alignment: .bottom) {
                Rectangle()
                    .fill(theme.colors.border)
                    .frame(height: 1)
            }

            // Navigation list
            ScrollView {
                VStack(alignment: .leading, spacing: 10) {
                    ForEach(SettingsCategoryGroup.allCases, id: \.self) { group in
                        VStack(alignment: .leading, spacing: 4) {
                            // Section label
                            Text(group.rawValue)
                                .font(.system(size: 10, weight: .semibold))
                                .tracking(1)
                                .foregroundStyle(theme.colors.textMuted)
                                .padding(.horizontal, 20)
                                .padding(.top, 4)

                            // Nav items
                            ForEach(group.categories) { category in
                                sidebarItem(category)
                            }
                        }
                    }
                }
                .padding(.vertical, 8)
            }
        }
        .background(theme.colors.background)
        .overlay(alignment: .trailing) {
            Rectangle()
                .fill(theme.colors.border)
                .frame(width: 1)
        }
    }

    private func sidebarItem(_ category: SettingsCategory) -> some View {
        let isSelected = selectedCategory == category
        return Button {
            selectedCategory = category
        } label: {
            HStack(spacing: 10) {
                Image(systemName: category.icon)
                    .font(.system(size: 14))
                    .foregroundStyle(isSelected ? theme.colors.primary : theme.colors.textMuted)
                    .frame(width: 18)

                Text(category.label)
                    .font(.system(size: 13, weight: isSelected ? .semibold : .regular))
                    .foregroundStyle(isSelected ? theme.colors.primary : theme.colors.text)

                Spacer()
            }
            .padding(.horizontal, 20)
            .padding(.vertical, 8)
            .background(
                isSelected ? theme.colors.border.opacity(0.8) : .clear
            )
        }
        .buttonStyle(.plain)
    }

    private var settingsHeader: some View {
        HStack(alignment: .bottom) {
            Text(selectedCategory?.label ?? "")
                .font(.system(size: 28, weight: .semibold))
                .foregroundStyle(theme.colors.text)
            Spacer()
        }
        .padding(.horizontal, 24)
        .padding(.bottom, 12)
        .frame(height: 80)
        .background(headerGradient)
        .overlay(alignment: .bottom) {
            Rectangle()
                .fill(theme.colors.border)
                .frame(height: 1)
        }
    }

    // MARK: - Content

    @ViewBuilder
    private var settingsContent: some View {
        switch selectedCategory {
        case nil:
            SettingsOverview(onNavigate: { selectedCategory = $0 })
        case .appearance:
            AppearancePanel()
        case .aiModel:
            AIModelPanel()
        case .behavior:
            BehaviorPanel()
        case .advanced:
            AdvancedPanel()
        case .privacy:
            PrivacyPanel()
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
                // Hero section
                VStack(spacing: 8) {
                    Text("How to change BoBe")
                        .font(.system(size: 24, weight: .bold))
                        .foregroundStyle(theme.colors.text)

                    Text("BoBe uses a personality (Soul), your goals, and memories to provide contextual, proactive assistance. Here's how to customize your experience.")
                        .font(.system(size: 14))
                        .foregroundStyle(theme.colors.textMuted)
                        .multilineTextAlignment(.center)
                        .frame(maxWidth: 480)
                }
                .padding(.top, 32)

                // Cards grid
                LazyVGrid(columns: [
                    GridItem(.flexible(), spacing: 16),
                    GridItem(.flexible(), spacing: 16),
                ], spacing: 16) {
                    overviewCard(
                        icon: "eye.fill",
                        color: theme.colors.primary,
                        heading: "What BoBe sees",
                        body: "Control screen capture, context window, and what information BoBe has access to.",
                        target: .behavior
                    )
                    overviewCard(
                        icon: "brain.head.profile",
                        color: theme.colors.secondary,
                        heading: "What BoBe remembers",
                        body: "Manage memories, conversation history, and how BoBe learns from interactions.",
                        target: .memories
                    )
                    overviewCard(
                        icon: "message.fill",
                        color: theme.colors.tertiary,
                        heading: "When BoBe speaks up",
                        body: "Configure check-in frequency, proactive messages, and notification preferences.",
                        target: .behavior
                    )
                    overviewCard(
                        icon: "paintbrush.fill",
                        color: theme.colors.primary.opacity(0.7),
                        heading: "How BoBe sounds",
                        body: "Choose and customize personality templates that shape BoBe's communication style.",
                        target: .souls
                    )
                    overviewCard(
                        icon: "bolt.fill",
                        color: theme.colors.secondary,
                        heading: "What BoBe can do",
                        body: "Enable tools, connect MCP servers, and extend BoBe's capabilities.",
                        target: .tools
                    )
                }
                .padding(.horizontal, 24)

                // Footer
                Text("Use the sidebar to explore all settings. Everything runs locally on your Mac.")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.colors.textMuted)
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
            onNavigate(target)
        } label: {
            HStack(spacing: 14) {
                Image(systemName: icon)
                    .font(.system(size: 22))
                    .foregroundStyle(color)
                    .frame(width: 36, height: 36)

                VStack(alignment: .leading, spacing: 4) {
                    Text(heading)
                        .font(.system(size: 14, weight: .semibold))
                        .foregroundStyle(theme.colors.text)

                    Text(body)
                        .font(.system(size: 12))
                        .foregroundStyle(theme.colors.textMuted)
                        .lineLimit(3)
                        .multilineTextAlignment(.leading)
                }

                Spacer()

                Image(systemName: "chevron.right")
                    .font(.system(size: 12))
                    .foregroundStyle(theme.colors.textMuted)
            }
            .padding(16)
            .background(
                RoundedRectangle(cornerRadius: 12)
                    .fill(theme.colors.background)
                    .overlay(
                        RoundedRectangle(cornerRadius: 12)
                            .stroke(theme.colors.border, lineWidth: 1)
                    )
                    .shadow(color: Color.black.opacity(0.06), radius: 4, y: 2)
            )
        }
        .buttonStyle(.plain)
    }
}
