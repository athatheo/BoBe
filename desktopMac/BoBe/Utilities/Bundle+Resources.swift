import Foundation

extension Bundle {
    /// Resolves the correct bundle for app resources.
    /// SPM generates `Bundle.module`; Xcode builds use `Bundle.main`.
    static var appResources: Bundle {
        #if SWIFT_PACKAGE
        return Bundle.module
        #else
        return Bundle.main
        #endif
    }
}
