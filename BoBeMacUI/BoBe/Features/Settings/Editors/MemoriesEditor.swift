import SwiftUI

/// Mirrors daemon `MemoryFile::DEFAULT_BODY`; used by Reset and PrivacyPanel nuke.
let memoryDefaultBody = """
# BoBe Memory

## Profile

## Active Goals

## Long-term

## Recent
"""

private let memoryTargetMaxBytes = 50 * 1024
private let memoryConsolidationDeltaThreshold = 200

struct MemoriesEditor: View {
    @State private var text = ""
    @State private var savedText = ""
    @State private var bytes = 0
    @State private var isLoading = false
    @State private var isSaving = false
    @State private var status: String?
    @State private var statusIsError = false
    @State private var consolidationHintShown = false
    @Environment(\.theme) private var theme

    private var isDirty: Bool {
        self.text != self.savedText
    }

    private var byteProgress: Double {
        let count = self.text.utf8.count
        return Double(count) / Double(memoryTargetMaxBytes)
    }

    private var byteLabel: String {
        let formatter = ByteCountFormatter()
        formatter.countStyle = .file
        formatter.allowedUnits = [.useKB, .useMB]
        let used = formatter.string(fromByteCount: Int64(self.text.utf8.count))
        let target = formatter.string(fromByteCount: Int64(memoryTargetMaxBytes))
        return L10n.tr("settings.memory.bytes_format", used, target)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            HStack {
                Text(L10n.tr("settings.memory.title"))
                    .font(.headline)
                    .foregroundStyle(self.theme.colors.text)
                Spacer()
            }
            .padding(.bottom, 4)

            Text(L10n.tr("settings.memory.description"))
                .font(.system(size: 12))
                .foregroundStyle(self.theme.colors.textMuted)

            self.editorBody
            self.byteGauge
            self.actionRow
            self.statusFooter
        }
        .padding(20)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .task { await self.load() }
    }

    private var editorBody: some View {
        Group {
            if self.isLoading, self.savedText.isEmpty {
                HStack(spacing: 8) {
                    BobeSpinner(size: 14)
                    Text(L10n.tr("settings.memories.loading"))
                        .font(.system(size: 13))
                        .foregroundStyle(self.theme.colors.textMuted)
                }
                .frame(maxWidth: .infinity, minHeight: 360, alignment: .center)
            } else {
                CodeEditor(text: self.$text, theme: self.theme, fontSize: 13)
                    .frame(minHeight: 360, maxHeight: .infinity)
                    .background(
                        RoundedRectangle(cornerRadius: 8)
                            .fill(self.theme.colors.surface)
                            .stroke(self.theme.colors.border, lineWidth: 1)
                    )
            }
        }
    }

    private var byteGauge: some View {
        HStack(spacing: 10) {
            BobeLinearProgressBar(progress: self.byteProgress, tint: self.gaugeTint)
                .frame(maxWidth: .infinity)
            Text(self.byteLabel)
                .font(.system(size: 11, design: .monospaced))
                .foregroundStyle(self.theme.colors.textMuted)
        }
    }

    /// Value-coded health color for the memory size gauge. A positional
    /// gradient would imply "more colorful = more memories", which has no
    /// meaning — what the user cares about is whether they're approaching
    /// the soft cap and should consolidate.
    private var gaugeTint: Color {
        switch self.byteProgress {
        case ..<0.75:
            self.theme.colors.secondary
        case ..<0.92:
            self.theme.colors.tertiary
        default:
            self.theme.colors.primary
        }
    }

    private var actionRow: some View {
        HStack(spacing: 8) {
            Button(L10n.tr("settings.memory.action.reload")) {
                Task { await self.load(force: true) }
            }
            .bobeButton(.secondary, size: .small)
            .disabled(self.isLoading || self.isSaving)

            Spacer()

            Button(L10n.tr("settings.memory.action.reset_default")) {
                self.text = memoryDefaultBody
            }
            .bobeButton(.secondary, size: .small)
            .disabled(self.isLoading || self.isSaving || self.text == memoryDefaultBody)

            Button(self.isSaving
                ? L10n.tr("settings.shared.action.saving")
                : L10n.tr("settings.shared.action.save")) {
                    Task { await self.save() }
                }
                .bobeButton(.primary, size: .small)
                .disabled(!self.isDirty || self.isSaving)
        }
    }

    @ViewBuilder
    private var statusFooter: some View {
        if let status {
            Text(status)
                .font(.system(size: 11))
                .foregroundStyle(self.statusIsError ? self.theme.colors.primary : self.theme.colors.secondary)
        } else if self.consolidationHintShown {
            HStack(spacing: 4) {
                Image(systemName: "moon.stars.fill")
                    .font(.system(size: 11))
                    .foregroundStyle(self.theme.colors.tertiary)
                Text(L10n.tr("settings.memory.consolidation_hint"))
                    .font(.system(size: 11))
                    .foregroundStyle(self.theme.colors.tertiary)
            }
        }
    }

    private func load(force: Bool = false) async {
        if !force, self.isDirty {
            self.statusIsError = false
            self.status = L10n.tr("settings.memory.discard_to_reload")
            return
        }
        self.isLoading = true
        defer { self.isLoading = false }
        do {
            let resp = try await DaemonClient.shared.getMemory()
            self.text = resp.content
            self.savedText = resp.content
            self.bytes = resp.bytes
            self.status = nil
            self.statusIsError = false
        } catch {
            self.status = error.localizedDescription
            self.statusIsError = true
        }
    }

    private func save() async {
        self.isSaving = true
        defer { self.isSaving = false }
        let sentLength = self.text.utf8.count
        do {
            let resp = try await DaemonClient.shared.updateMemory(self.text)
            self.savedText = resp.content
            self.text = resp.content
            self.bytes = resp.bytes
            self.statusIsError = false
            self.status = L10n.tr("settings.memory.saved")
            self.consolidationHintShown = abs(resp.bytes - sentLength) > memoryConsolidationDeltaThreshold
        } catch {
            self.status = error.localizedDescription
            self.statusIsError = true
        }
    }
}
