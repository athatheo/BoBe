import SwiftUI

/// Settings category for sidebar navigation
enum SettingsCategory: String, CaseIterable, Identifiable {
    case overview
    case souls, goals, memories, userProfiles = "user-profiles"
    case tools, mcpServers = "mcp-servers"
    case appearance, aiModel = "ai-model", behavior, goalWorker = "goal-worker", privacy
    case advanced

    var id: String { rawValue }

    var label: String {
        switch self {
        case .overview: "Overview"
        case .souls: "Souls"
        case .goals: "Goals"
        case .memories: "Memories"
        case .userProfiles: "User Profiles"
        case .tools: "Tools"
        case .mcpServers: "MCP Servers"
        case .appearance: "Appearance"
        case .aiModel: "AI Model"
        case .behavior: "Behavior"
        case .goalWorker: "Goal Worker"
        case .privacy: "Privacy"
        case .advanced: "For Nerds"
        }
    }

    var icon: String {
        switch self {
        case .overview: "house.fill"
        case .souls: "sparkles"
        case .goals: "target"
        case .memories: "brain.head.profile"
        case .userProfiles: "person.fill"
        case .tools: "wrench.fill"
        case .mcpServers: "server.rack"
        case .appearance: "paintpalette.fill"
        case .aiModel: "cpu.fill"
        case .behavior: "slider.horizontal.3"
        case .goalWorker: "play.circle.fill"
        case .privacy: "shield.fill"
        case .advanced: "terminal.fill"
        }
    }

    var group: SettingsCategoryGroup? {
        switch self {
        case .overview: nil
        case .souls, .goals, .memories, .userProfiles: .context
        case .tools, .mcpServers: .integrations
        case .appearance, .aiModel, .behavior, .goalWorker, .privacy: .preferences
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
        SettingsCategory.allCases.filter { $0.group == self }
    }
}

/// Main settings window view with sidebar + content
struct SettingsWindow: View {
    @State private var selectedCategory: SettingsCategory? = .overview
    @Environment(\.theme) private var theme

    var body: some View {
        NavigationSplitView {
            settingsSidebar
                .navigationSplitViewColumnWidth(min: 200, ideal: 220, max: 240)
        } detail: {
            settingsContent
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(theme.colors.background)
        }
    }

    // MARK: - Sidebar (matches settings-sidebar CSS)

    private var settingsSidebar: some View {
        VStack(spacing: 0) {
            // Header (80px with gradient)
            VStack(spacing: 4) {
                Text("BoBe")
                    .font(.system(size: 20, weight: .bold))
                    .foregroundStyle(theme.colors.primary)
                Text("Settings")
                    .font(.system(size: 12, weight: .medium))
                    .foregroundStyle(theme.colors.textMuted)
            }
            .frame(maxWidth: .infinity)
            .frame(height: 80)
            .background(
                LinearGradient(
                    colors: [theme.colors.background, theme.colors.border.opacity(0.3)],
                    startPoint: .top,
                    endPoint: .bottom
                )
            )

            // Navigation list
            ScrollView {
                VStack(alignment: .leading, spacing: 16) {
                    ForEach(SettingsCategoryGroup.allCases, id: \.self) { group in
                        VStack(alignment: .leading, spacing: 4) {
                            // Section label
                            Text(group.rawValue)
                                .font(.system(size: 10, weight: .semibold))
                                .tracking(1)
                                .foregroundStyle(theme.colors.textMuted)
                                .padding(.horizontal, 12)
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
                    .foregroundStyle(isSelected ? theme.colors.text : theme.colors.textMuted)

                Spacer()
            }
            .padding(.horizontal, 12)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(isSelected ? theme.colors.primary.opacity(0.1) : .clear)
            )
            .padding(.horizontal, 8)
        }
        .buttonStyle(.plain)
    }

    // MARK: - Content

    @ViewBuilder
    private var settingsContent: some View {
        switch selectedCategory {
        case .overview, nil:
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
        case .goalWorker:
            GoalWorkerPanel()
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
                        color: Color(hex: "C67B5C"),
                        heading: "What BoBe sees",
                        body: "Control screen capture, context window, and what information BoBe has access to.",
                        target: .behavior
                    )
                    overviewCard(
                        icon: "brain.head.profile",
                        color: Color(hex: "8B9A7D"),
                        heading: "What BoBe remembers",
                        body: "Manage memories, conversation history, and how BoBe learns from interactions.",
                        target: .memories
                    )
                    overviewCard(
                        icon: "message.fill",
                        color: Color(hex: "D4A574"),
                        heading: "When BoBe speaks up",
                        body: "Configure check-in frequency, proactive messages, and notification preferences.",
                        target: .behavior
                    )
                    overviewCard(
                        icon: "paintbrush.fill",
                        color: Color(hex: "A69080"),
                        heading: "How BoBe sounds",
                        body: "Choose and customize personality templates that shape BoBe's communication style.",
                        target: .souls
                    )
                    overviewCard(
                        icon: "bolt.fill",
                        color: Color(hex: "8B9A7D"),
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
                    .shadow(color: Color(hex: "3A3A3A").opacity(0.04), radius: 4, y: 2)
            )
        }
        .buttonStyle(.plain)
    }
}
