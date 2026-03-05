import Foundation

extension Bundle {
    /// `Bundle.module` for SPM, `Bundle.main` for Xcode builds.
    static var appResources: Bundle {
        #if SWIFT_PACKAGE
            return Bundle.module
        #else
            return Bundle.main
        #endif
    }
}
