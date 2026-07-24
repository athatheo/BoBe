import Foundation

/// Errors thrown by `DaemonClient` operations. Categorized so callers
/// can distinguish "daemon not reachable" from "request was bad".
enum DaemonError: Error, LocalizedError {
    case invalidResponse
    case httpError(statusCode: Int, message: String, code: String? = nil)
    case connectionFailed
    case operationFailed(String)

    var errorDescription: String? {
        switch self {
        case .invalidResponse: "Invalid response from daemon"
        case let .httpError(code, msg, _): "HTTP \(code): \(msg)"
        case .connectionFailed: "Failed to connect to daemon"
        case let .operationFailed(message): message
        }
    }
}

/// Type-erased `Encodable` wrapper for request bodies whose concrete type
/// the call site doesn't need to expose. Used by the JSON `fetch` helper
/// so signatures stay generic without dragging the type parameter through.
struct AnyEncodable: Encodable {
    private let encode: (Encoder) throws -> Void

    init(_ wrapped: any Encodable) {
        self.encode = wrapped.encode
    }

    func encode(to encoder: Encoder) throws {
        try self.encode(encoder)
    }
}
