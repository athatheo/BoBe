@testable import BoBe
import Foundation
import Testing

@Suite("Settings PATCH encoding")
struct SettingsPatchEncodingTests {
    @Test
    func mcpEnabledEncodesAsDaemonField() throws {
        var request = SettingsUpdateRequest()
        request.mcpEnabled = false

        let encoded = try #require(
            JSONSerialization.jsonObject(with: JSONEncoder().encode(request)) as? [String: Any]
        )

        #expect(encoded["mcp_enabled"] as? Bool == false)
    }

    @Test
    func nullableFieldsEncodeUnchangedValueAndClearDistinctly() throws {
        let encoder = JSONEncoder()

        let unchanged = try #require(
            JSONSerialization.jsonObject(with: encoder.encode(SettingsUpdateRequest())) as? [String: Any]
        )
        #expect(!unchanged.keys.contains("provider_base_url"))

        var valueRequest = SettingsUpdateRequest()
        valueRequest.providerBaseUrl = .value("http://localhost:11434/v1")
        let value = try #require(
            JSONSerialization.jsonObject(with: encoder.encode(valueRequest)) as? [String: Any]
        )
        #expect(value["provider_base_url"] as? String == "http://localhost:11434/v1")

        var clearRequest = SettingsUpdateRequest()
        clearRequest.providerBaseUrl = .clear
        let clear = try #require(
            JSONSerialization.jsonObject(with: encoder.encode(clearRequest)) as? [String: Any]
        )
        #expect(clear.keys.contains("provider_base_url"))
        #expect(clear["provider_base_url"] is NSNull)
    }
}
