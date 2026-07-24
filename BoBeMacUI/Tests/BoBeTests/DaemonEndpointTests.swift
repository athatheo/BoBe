@testable import BoBe
import Foundation
import Testing

@Suite("Daemon endpoint")
struct DaemonEndpointTests {
    @Test
    func remoteEndpointAddsBearerAuthorization() throws {
        let url = try #require(URL(string: "https://bobe.example.test"))
        let endpoint = DaemonEndpoint.remote(baseURL: url, bearerToken: "secret-token")
        var request = URLRequest(url: url)

        endpoint.authorize(&request)

        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer secret-token")
    }

    @Test
    func managedLocalEndpointDoesNotAddAuthorization() throws {
        let url = try #require(URL(string: "http://127.0.0.1:8766"))
        var request = URLRequest(url: url)

        DaemonEndpoint.managedLocal.authorize(&request)

        #expect(request.value(forHTTPHeaderField: "Authorization") == nil)
    }

    @Test
    func queryItemsArePercentEncoded() async {
        let url = await DaemonClient.shared.endpointURL(
            "/models",
            queryItems: [URLQueryItem(name: "engine", value: "local&debug=true")]
        )

        #expect(url.query == "engine=local%26debug%3Dtrue")
    }

    @Test
    func messageRequestCarriesStableIdempotencyKey() throws {
        let requestId = try #require(UUID(uuidString: "11111111-2222-3333-4444-555555555555"))
        let encoded = try #require(
            JSONSerialization.jsonObject(
                with: JSONEncoder().encode(
                    SendMessageRequest(content: "hello", requestId: requestId)
                )
            ) as? [String: Any]
        )

        #expect(encoded["content"] as? String == "hello")
        #expect(
            (encoded["request_id"] as? String)?.lowercased()
                == requestId.uuidString.lowercased()
        )
    }

    @Test
    func copilotCLIUsesExplicitExecutableOverride() {
        let bundleURL = URL(fileURLWithPath: "/Applications/BoBe.app", isDirectory: true)
        let configuredPath = "/tmp/project-tools/copilot"

        let resolved = BackendService.resolveCopilotCLIPath(
            environment: ["COPILOT_CLI_PATH": configuredPath],
            bundleURL: bundleURL,
            isExecutableFile: { $0 == configuredPath }
        )

        #expect(resolved == configuredPath)
    }

    @Test
    func copilotCLIFallsBackToSignedBundleHelper() {
        let bundleURL = URL(fileURLWithPath: "/Applications/BoBe.app", isDirectory: true)
        let helperPath = "/Applications/BoBe.app/Contents/Helpers/copilot"

        let resolved = BackendService.resolveCopilotCLIPath(
            environment: [:],
            bundleURL: bundleURL,
            isExecutableFile: { $0 == helperPath }
        )

        #expect(resolved == helperPath)
    }

    @Test
    func copilotCLIPrefersSignedBundleHelperOverEnvironment() {
        let bundleURL = URL(fileURLWithPath: "/Applications/BoBe.app", isDirectory: true)
        let helperPath = "/Applications/BoBe.app/Contents/Helpers/copilot"

        let resolved = BackendService.resolveCopilotCLIPath(
            environment: ["COPILOT_CLI_PATH": "/tmp/alternate-copilot"],
            bundleURL: bundleURL,
            isExecutableFile: { _ in true }
        )

        #expect(resolved == helperPath)
    }

    @Test
    func copilotCLIRejectsMissingExecutables() {
        let resolved = BackendService.resolveCopilotCLIPath(
            environment: ["COPILOT_CLI_PATH": "/missing/copilot"],
            bundleURL: URL(fileURLWithPath: "/Applications/BoBe.app", isDirectory: true),
            isExecutableFile: { _ in false }
        )

        #expect(resolved == nil)
    }

    @Test
    func terminalLoginDisablesUpdatesAndEscapesHelperPath() {
        let command = CopilotSignIn.loginCommand(cliPath: "/Applications/John's BoBe.app/copilot")

        #expect(
            command
                == "'/Applications/John'\\''s BoBe.app/copilot' --no-auto-update login"
        )
    }

    @Test
    func terminalLoginPATHFallbackDisablesUpdates() {
        #expect(CopilotSignIn.loginCommand(cliPath: nil) == "copilot --no-auto-update login")
    }
}
