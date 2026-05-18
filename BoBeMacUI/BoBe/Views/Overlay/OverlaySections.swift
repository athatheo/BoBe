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
        .onHover { hovering in
            // Keep the chat open while the cursor is inside the overlay
            // bounds — otherwise the inactivity timer dismisses a long
            // message while the user is still reading it.
            if self.isChatVisible {
                self.isPointerOverChat = hovering
                if hovering {
                    self.lastMessageActivity = .now
                }
            }
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
                    // MicButton intentionally NOT here — there is exactly
                    // ONE mic in the UI, anchored to the avatar's lower-left
                    // circumference, present whether the chat is open or
                    // closed. Having a second mic in the composer made the
                    // affordance appear to "teleport" when the chat opened.
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
            let bannerColor = self.themeStore.currentTheme.colors.background
            HStack(spacing: 8) {
                Image(systemName: "bolt.slash.fill")
                    .font(.system(size: 11))
                Text(L10n.tr("overlay.error.daemon_disconnected"))
                    .bobeTextStyle(.overlayStatus)
                    .lineLimit(1)
                Spacer()
                Button(L10n.tr("overlay.error.daemon_restart")) {
                    Task {
                        try? await BackendService.shared.userRestart()
                    }
                }
                .buttonStyle(.plain)
                .padding(.horizontal, 8)
                .padding(.vertical, 3)
                .background(
                    RoundedRectangle(cornerRadius: 6)
                        .fill(bannerColor.opacity(0.2))
                )
                .accessibilityLabel(L10n.tr("overlay.error.daemon_restart.accessibility"))
            }
            .foregroundStyle(bannerColor)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.85)))
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        } else if let error = self.store.errorMessage {
            let bannerColor = self.themeStore.currentTheme.colors.background
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
            .foregroundStyle(bannerColor)
            .padding(.horizontal, 10)
            .padding(.vertical, 6)
            .background(RoundedRectangle(cornerRadius: 8).fill(.red.opacity(0.85)))
            .padding(.horizontal, 12)
            .transition(self.overlaySectionTransition)
        }
    }

    var avatarSection: some View {
        // Vertical stack: bubble slot ABOVE the avatar so it has full
        // horizontal room to grow. Anchored bottom-right of the overlay.
        // .onHover on the whole stack drives the satellite reveal — the
        // mic and chat toggle stay hidden at rest, fade in when the
        // cursor enters the avatar area. .contentShape forces hit-testing
        // across the whole frame, including transparent gaps between
        // bubble and avatar.
        VStack(alignment: .trailing, spacing: 2) {
            self.satelliteSlot
                .frame(maxWidth: 340, alignment: .trailing)

            self.avatarCluster
        }
        .padding(.trailing, 12)
        .padding(.bottom, 8)
        .padding(.top, 0)
        .contentShape(Rectangle())
        .onHover { isHovering in
            withAnimation(.easeOut(duration: 0.15)) {
                self.avatarAreaHovered = isHovering
            }
        }
        // Single .animation keyed off a composite token instead of three
        // stacked `.animation(value:)` modifiers. With the stack, only the
        // last modifier wins on any single render pass — when two source
        // values changed in the same pass (e.g. silence toggle dismisses
        // the floating bubble), the bubble id change effectively got the
        // wrong animation. Composite-token keying lets one transaction
        // cover all three transitions.
        .animation(.easeOut(duration: 0.2), value: self.avatarAnimationToken)
        .zIndex(3)
    }

    /// Composite identity for the avatar section's animation modifier.
    /// Any change to one of the three drivers re-evaluates the string and
    /// triggers the animation exactly once for that frame.
    private var avatarAnimationToken: String {
        let bubbleId = self.floatingBubbleMessage?.id ?? "·"
        return "\(bubbleId)|\(self.store.proactiveSilenced)|\(self.isChatVisible)"
    }

    /// Content above the avatar. Hosts either the voice coachmark (a
    /// one-shot teaching moment) or the consolidated AvatarStatusBubble
    /// — never both simultaneously. The bubble itself enforces priority
    /// across its sub-modes (BoBe message / user STT / status / silenced)
    /// so this slot only needs the coachmark/bubble choice.
    @ViewBuilder
    var satelliteSlot: some View {
        if self.voiceCoachmarkVisible, !self.isChatVisible {
            VoiceCoachmark(isVisible: self.$voiceCoachmarkVisible)
        } else {
            AvatarStatusBubble(
                mode: self.bubbleMode,
                onOpenChat: { self.openChatManually() },
                onToggleSilence: { self.toggleProactiveSilence() },
                onDismiss: { self.dismissFloatingBubble() },
                isSilenced: self.store.proactiveSilenced,
                isTtsAudible: self.isTtsAudible
            )
        }
    }

    /// The avatar + its satellites (chat toggle, mic). Satellites are
    /// owned by `AvatarView` itself via the `cardSatellites:` parameter,
    /// so their positions are expressed in the card's local coordinate
    /// space (0..cardSize) — they CAN'T drift when outer layout
    /// constants (column size, padding, BobeLabel offset) change.
    ///
    /// The chat toggle and mic are hover-gated — at rest the avatar
    /// reads as a single clean circle; on hover the satellites fade in.
    /// The chat toggle stays visible while chat is open so the user
    /// never loses the close affordance.
    var avatarCluster: some View {
        // 32pt diameter satellite buttons. Chat toggle at 10:30, mic
        // at 7:30 (clockwise from 12 = 315° and 225°). The math runs
        // entirely on `AvatarMetrics.cardSize` — no column dependencies.
        let satSize: CGFloat = 32
        let chatOffset = AvatarMetrics.rimOffset(angleFrom12: 315, satelliteSize: satSize)
        let micOffset = AvatarMetrics.rimOffset(angleFrom12: 225, satelliteSize: satSize)

        // Chat-open: leave headroom for the StatusLabel above the
        // avatar head. Chat-closed: bubble owns status — drop the
        // headroom so the bubble nestles tight against the avatar.
        let topInset: CGFloat = self.isChatVisible ? 18 : 4
        let leadingPad: CGFloat = 16
        let clusterHeight: CGFloat = self.isChatVisible ? 164 : 148

        return AvatarView(
            stateType: self.avatarStateType,
            isConnected: self.store.isConnected,
            hasMessage: self.hasUnreadMessages,
            showInput: self.isChatVisible,
            statusOverride: self.statusTextOverride,
            showStatusLabel: self.isChatVisible,
            bubbleShowingMessage: !self.isChatVisible && self.floatingBubbleMessage != nil,
            isAvatarActionEnabled: self.canAvatarToggleChat,
            onClick: self.avatarClickAction,
            onToggleCapture: self.handleCaptureToggle,
            cardSatellites: {
                if self.showChatToggleSatellite {
                    ChatToggleButton(isActive: self.isChatVisible, action: self.toggleChatFromBubble)
                        .offset(chatOffset)
                        .transition(.opacity)
                }
                if self.showMicSatellite {
                    MicButton()
                        .offset(micOffset)
                        .transition(.opacity)
                }
            }
        )
        .padding(.top, topInset)
        .padding(.leading, leadingPad)
        .frame(width: 148, height: clusterHeight, alignment: .topLeading)
        .animation(.easeOut(duration: 0.15), value: self.showChatToggleSatellite)
        .animation(.easeOut(duration: 0.15), value: self.showMicSatellite)
        .animation(.easeOut(duration: 0.2), value: self.isChatVisible)
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
