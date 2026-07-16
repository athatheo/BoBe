@testable import BoBe
import Foundation
import Testing

@Suite("Client TTS engine", .serialized)
struct ClientTtsEngineTests {
    @Test
    func boundedSentenceQueuePreservesOrderUnderBackpressure() async throws {
        let queue = ClientTtsSentenceQueue(capacity: 1)
        let first = ClientTtsSentence(turnId: "turn", sequence: 0, text: "first")
        let second = ClientTtsSentence(turnId: "turn", sequence: 1, text: "second")

        #expect(await queue.send(first))
        let secondSend = Task { await queue.send(second) }
        await Task.yield()

        #expect(await queue.next()?.sequence == 0)
        #expect(await secondSend.value)
        await queue.finish()
        #expect(await queue.next()?.sequence == 1)
        #expect(await queue.next() == nil)
    }

    @Test
    func cancellingSentenceQueueRejectsBlockedProducer() async throws {
        let queue = ClientTtsSentenceQueue(capacity: 1)
        let first = ClientTtsSentence(turnId: "turn", sequence: 0, text: "first")
        let second = ClientTtsSentence(turnId: "turn", sequence: 1, text: "second")

        #expect(await queue.send(first))
        let secondSend = Task { await queue.send(second) }
        await Task.yield()
        await queue.cancel()

        #expect(!(await secondSend.value))
        #expect(await queue.next() == nil)
    }

    @Test
    func realSupertonicSynthesisWhenEnabled() async throws {
        guard ProcessInfo.processInfo.environment["BOBE_RUN_MODEL_TESTS"] == "1" else {
            return
        }
        let engine = ClientTtsEngine()
        try await engine.prepare()
        let started = ContinuousClock.now
        let samples = try await engine.synthesize(
            text: "BoBe client speech is ready.",
            language: "en",
            voiceId: "af_bella",
            speed: 1
        )
        #expect(samples.count > 24_000 / 2)
        #expect(samples.contains(where: { $0 != 0 }))
        #expect(started.duration(to: .now) < .seconds(1))
        await engine.cleanup()
    }
}
