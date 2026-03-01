import AppKit
import Foundation
import OSLog
import Sparkle

private let updaterLogger = Logger(subsystem: "com.bobe.app", category: "Updater")

/// Wraps Sparkle updater lifecycle and exposes update actions to the UI.
@MainActor
final class UpdaterManager: NSObject, SPUUpdaterDelegate {
    static let shared = UpdaterManager()

    private lazy var controller = SPUStandardUpdaterController(
        startingUpdater: true,
        updaterDelegate: self,
        userDriverDelegate: nil
    )

    override private init() {
        super.init()
    }

    var isConfigured: Bool {
        self.feedURLString != nil
    }

    var canCheckForUpdates: Bool {
        self.isConfigured && self.controller.updater.canCheckForUpdates
    }

    func setup() {
        _ = self.controller
        self.controller.updater.clearFeedURLFromUserDefaults()
    }

    func checkForUpdates() {
        guard self.canCheckForUpdates else {
            if !self.isConfigured {
                updaterLogger.warning("Updates not configured: missing or placeholder SUFeedURL")
            }
            return
        }
        self.controller.checkForUpdates(nil)
    }

    func feedURLString(for updater: SPUUpdater) -> String? {
        self.feedURLString
    }

    private var feedURLString: String? {
        guard let rawFeedURL = Bundle.main.object(forInfoDictionaryKey: "SUFeedURL") as? String else {
            return nil
        }
        let trimmed = rawFeedURL.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, trimmed != "https://example.com/appcast.xml" else {
            return nil
        }
        return trimmed
    }
}
