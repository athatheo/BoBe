import SwiftUI

extension OverlayView {
    var overlayContent: some View {
        VStack(spacing: 0) {
            self.chatHistorySection
            self.conversationEndingSection
            self.recoverySection
            self.composerSection
            self.softWarningSection
            self.errorBannerSection
            self.avatarSection
        }
    }

    @ViewBuilder
    var conversationEndingSection: some View {
        if self.isChatVisible, self.store.conversationEnding {
            HStack(spacing: 6) {
                Image(systemName: "moon.fill")
                    .font(.system(size: 9))
                Text(L10n.tr("overlay.conversation.ending"))
                    .bobeTextStyle(.overlayStatus)
                Spacer()
            }
            .foregroundStyle(self.themeStore.currentTheme.colors.textMuted)
            .padding(.horizontal, 14)
            .padding(.vertical, 4)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    var softWarningSection: some View {
        if let warning = self.store.softWarning {
            HStack(spacing: 6) {
                Image(systemName: "info.circle.fill")
                    .font(.system(size: 10))
                Text(warning)
                    .bobeTextStyle(.overlayStatus)
                    .lineLimit(2)
                Spacer()
                Button {
                    self.store.dismissSoftWarning()
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 8, weight: .bold))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L10n.tr("overlay.input.close.accessibility"))
            }
            .foregroundStyle(self.themeStore.currentTheme.colors.tertiary)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(
                RoundedRectangle(cornerRadius: 8)
                    .fill(self.themeStore.currentTheme.colors.tertiary.opacity(0.12))
            )
            .overlay(
                RoundedRectangle(cornerRadius: 8)
                    .stroke(self.themeStore.currentTheme.colors.tertiary.opacity(0.4), lineWidth: 1)
            )
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    var chatHistorySection: some View {
        if self.isChatVisible, !self.store.messages.isEmpty {
            ChatStack(
                messages: self.store.messages,
                maxViewportHeight: self.chatViewportMaxHeight
            )
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    var recoverySection: some View {
        if self.isChatVisible, !self.store.failedSendRecoveries.isEmpty {
            VStack(spacing: 8) {
                ForEach(self.store.failedSendRecoveries) { recovery in
                    FailedSendRecoveryBanner(
                        recovery: recovery,
                        onRetry: {
                            Task { await self.store.retryFailedSendRecovery(recovery.id) }
                        },
                        onDismiss: {
                            self.store.dismissFailedSendRecovery(recovery.id)
                        }
                    )
                }
            }
            .padding(.horizontal, 12)
            .padding(.bottom, 8)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    var composerSection: some View {
        if self.isChatVisible {
            VStack(alignment: .trailing, spacing: 4) {
                VoicePartialCaption()
                HStack(alignment: .center, spacing: 8) {
                    MessageInput(
                        text: self.$draftMessage,
                        onSend: self.handleSendMessage,
                        onClose: { self.closeChat(userInitiated: true) },
                        feedbackMessage: self.composerFeedback,
                        isBusy: self.store.composerBlockReason != nil
                    )
                    .layoutPriority(1)
                    StopButton()
                    MicButton()
                }
            }
            .padding(.horizontal, 12)
            .zIndex(1)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    var errorBannerSection: some View {
        if self.store.context.daemonError {
            HStack(spacing: 8) {
                Image(systemName: "bolt.slash.fill")
                    .font(.system(size: 11))
                Text("Daemon disconnected")
                    .bobeTextStyle(.overlayStatus)
                    .lineLimit(1)
                Spacer()
                Button("Restart") {
                    Task {
                        try? await BackendService.shared.userRestart()
                    }
                }
                .buttonStyle(.plain)
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(.white.opacity(0.2))
                )
                .accessibilityLabel("Restart daemon")
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.85)))
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        } else if let error = self.store.errorMessage {
            HStack(spacing: 6) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 10))
                Text(error)
                    .bobeTextStyle(.overlayStatus)
                    .lineLimit(2)
                Spacer()
                Button {
                    self.store.dismissError()
                } label: {
                    Image(systemName: "xmark")
                        .font(.system(size: 8, weight: .bold))
                }
                .buttonStyle(.plain)
                .accessibilityLabel(L10n.tr("overlay.input.close.accessibility"))
            }
            .foregroundStyle(.white)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.85)))
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        }
    }

    @ViewBuilder
    var avatarSection: some View {
        HStack(spacing: 12) {
            Spacer()

            ZStack(alignment: .topLeading) {
                ChatToggleButton(isActive: self.isChatVisible, action: self.toggleChatFromBubble)
                    .padding(.leading, 11)
                    .padding(.top, 23)
                    .zIndex(4)

                AvatarView(
                    stateType: self.avatarStateType,
                    isCapturing: self.store.isCapturing,
                    isConnected: self.store.isConnected,
                    hasMessage: self.hasUnreadMessages,
                    showInput: self.isChatVisible,
                    statusOverride: self.statusTextOverride,
                    isAvatarActionEnabled: self.canAvatarToggleChat,
                    onClick: self.avatarClickAction,
                    onToggleCapture: self.handleCaptureToggle
                )
                .padding(.top, 18)
                .padding(.leading, 16)

                if !self.store.runningTools.isEmpty {
                    self.toolExecutionBadge
                        .padding(.top, 14)
                        .padding(.leading, 90)
                        .zIndex(5)
                }
            }
            .frame(width: 148, height: 164, alignment: .topLeading)
        }
        .padding(.trailing, 12)
        .padding(.bottom, 8)
        .padding(.top, 0)
        .zIndex(3)
    }

    var toolExecutionBadge: some View {
        let theme = self.themeStore.currentTheme
        let count = self.store.runningTools.count
        return HStack(spacing: 3) {
            Image(systemName: "wrench.fill")
                .font(.system(size: 9, weight: .bold))
            if count > 1 {
                Text("\(count)")
                    .font(.system(size: 9, weight: .bold, design: .monospaced))
            }
        }
        .foregroundStyle(theme.colors.background)
        .padding(.horizontal, 5)
        .padding(.vertical, 3)
        .background(Capsule().fill(theme.colors.primary))
        .overlay(Capsule().stroke(theme.colors.background, lineWidth: 1.5))
        .accessibilityLabel(L10n.tr("overlay.tool_badge.accessibility_format", count))
    }
}

struct FailedSendRecoveryBanner: View {
    let recovery: FailedSendRecovery
    let onRetry: () -> Void
    let onDismiss: () -> Void

    @Environment(\.theme) private var theme

    var body: some View {
        HStack(alignment: .top, spacing: 10) {
            Image(systemName: "exclamationmark.arrow.trianglehead.counterclockwise")
                .font(.system(size: 12, weight: .semibold))
                .foregroundStyle(self.theme.colors.primary)
                .padding(.top, 2)

            Text(self.recovery.content)
                .bobeTextStyle(.chatBody)
                .foregroundStyle(self.theme.colors.text)
                .lineLimit(2)
                .multilineTextAlignment(.leading)
                .frame(maxWidth: .infinity, alignment: .leading)

            Button(L10n.tr("app.common.retry"), action: self.onRetry)
                .bobeTextStyle(.rowMeta)
                .buttonStyle(.plain)
                .foregroundStyle(self.theme.colors.primary)

            Button(action: self.onDismiss) {
                Image(systemName: "xmark")
                    .font(.system(size: 9, weight: .bold))
                    .foregroundStyle(self.theme.colors.textMuted)
            }
            .buttonStyle(.plain)
            .accessibilityLabel(L10n.tr("overlay.input.close.accessibility"))
        }
        .padding(.horizontal, 10)
        .padding(.vertical, 8)
        .background(
            RoundedRectangle(cornerRadius: 12)
                .fill(self.theme.colors.surface)
        )
        .overlay(
            RoundedRectangle(cornerRadius: 12)
                .stroke(self.theme.colors.border, lineWidth: 1)
        )
    }
}
