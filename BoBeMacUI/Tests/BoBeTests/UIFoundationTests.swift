@testable import BoBe
import Testing

@Suite("UI foundations", .serialized)
struct UIFoundationTests {
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
}
