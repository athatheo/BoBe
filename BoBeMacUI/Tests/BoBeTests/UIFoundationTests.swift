@testable import BoBe
import Testing

@Suite("UI foundations", .serialized)
struct UIFoundationTests {
    @Test
    @MainActor
    func modelChangeClearsUnsupportedReasoning() {
        #expect(EnginePanel.retainedReasoningEffort("high", supported: ["low", "medium"]) == "")
        #expect(EnginePanel.retainedReasoningEffort("medium", supported: ["low", "medium"]) == "medium")
        #expect(EnginePanel.retainedReasoningEffort(nil, supported: ["low"]) == "")
    }

    @Test
    @MainActor
    func settingsWindowUsesProductiveInitialSize() {
        let size = SettingsWindowManager.initialSize(
            for: .init(x: 0, y: 0, width: 1512, height: 945)
        )
        #expect(size.width >= 1200)
        #expect(size.height >= 800)

        let constrained = SettingsWindowManager.initialSize(
            for: .init(x: 0, y: 0, width: 1024, height: 640)
        )
        #expect(constrained.width <= 984)
        #expect(constrained.height <= 600)
    }

    @Test
    @MainActor
    func unsavedCoordinatorDefersAndDiscardsTransition() {
        let coordinator = SettingsEditCoordinator.shared
        var discarded = false
        var transitioned = false
        coordinator.register(
            isDirty: true,
            save: { true },
            discard: { discarded = true }
        )
        coordinator.requestTransition { transitioned = true }
        #expect(coordinator.showsConfirmation)
        #expect(!transitioned)
        coordinator.discardAndContinue()
        #expect(discarded)
        #expect(transitioned)
        #expect(!coordinator.showsConfirmation)
    }

    @Test
    @MainActor
    func unsavedCoordinatorCancelKeepsDraftAndDestination() {
        let coordinator = SettingsEditCoordinator.shared
        var transitioned = false
        coordinator.register(isDirty: true, save: { true }, discard: {})
        coordinator.requestTransition { transitioned = true }
        coordinator.cancelTransition()
        #expect(!transitioned)
        #expect(coordinator.isDirty)
    }

    @Test
    func ambientMessageDwellHasNoReadingTimeCap() {
        let short = AmbientMessageTiming.dwellDuration(for: "Okay.")
        let medium = AmbientMessageTiming.dwellDuration(
            for: "I found the issue and updated the relevant settings. The complete explanation remains available so you can read it without rushing."
        )
        let long = AmbientMessageTiming.dwellDuration(for: String(repeating: "word ", count: 200))

        #expect(short == AmbientMessageTiming.minimumDwellSeconds)
        #expect(medium > short)
        #expect(long > medium)
    }

    @Test
    func onlyBriefProactiveMessagesAutoDismiss() {
        let requested = ChatMessage(
            sender: .bobe,
            content: "Requested answer",
            responseOrigin: .userInitiated
        )
        let proactive = ChatMessage(
            sender: .bobe,
            content: "A brief proactive note",
            responseOrigin: .proactive
        )
        let longProactive = ChatMessage(
            sender: .bobe,
            content: String(repeating: "word ", count: 61),
            responseOrigin: .proactive
        )

        #expect(!AmbientMessageTiming.shouldAutoDismiss(requested))
        #expect(AmbientMessageTiming.shouldAutoDismiss(proactive))
        #expect(!AmbientMessageTiming.shouldAutoDismiss(longProactive))
    }

    @Test
    func emptyAssistantPlaceholderDoesNotCreateConversationTrace() {
        let placeholder = ChatMessage(sender: .bobe, content: "")
        let streaming = ChatMessage(sender: .bobe, content: "", isStreaming: true)

        #expect(!placeholder.belongsInConversationTrace)
        #expect(streaming.belongsInConversationTrace)
    }
}
