@preconcurrency import AVFoundation
import Foundation

/// Decodes raw Opus packets (one packet at a time) into PCM via
/// `AVAudioConverter` + `kAudioFormatOpus`. Replaces `swift-opus`'s
/// `Opus.Decoder` so we don't drag an unmaintained SPM dep for what
/// macOS already covers natively (Apple's bundled libopus, available
/// since macOS 10.13).
///
/// Each `decode(_:to:)` call consumes one packet and emits up to
/// `frameCapacity` PCM frames. Opus packets are 2.5/5/10/20/40/60ms;
/// BoBe's daemon emits 20ms at 24kHz mono (480 samples/packet).
final class OpusDecoder {
    private let converter: AVAudioConverter
    private let inputFormat: AVAudioFormat

    enum DecodeError: Error {
        case invalidInputFormat
        case converterUnavailable
        case decodeFailed(NSError?)
    }

    init(outputFormat: AVAudioFormat) throws {
        // Source ASBD: Opus, VBR (variable bytes + frames per packet).
        // Apple's libopus reads the frame count from each packet's TOC byte.
        var asbd = AudioStreamBasicDescription(
            mSampleRate: outputFormat.sampleRate,
            mFormatID: kAudioFormatOpus,
            mFormatFlags: 0,
            mBytesPerPacket: 0,
            mFramesPerPacket: 0,
            mBytesPerFrame: 0,
            mChannelsPerFrame: outputFormat.channelCount,
            mBitsPerChannel: 0,
            mReserved: 0
        )
        guard let opus = AVAudioFormat(streamDescription: &asbd) else {
            throw DecodeError.invalidInputFormat
        }
        guard let converter = AVAudioConverter(from: opus, to: outputFormat) else {
            throw DecodeError.converterUnavailable
        }
        self.inputFormat = opus
        self.converter = converter
    }

    /// Decode one Opus packet. `buffer` is filled to its `frameCapacity`
    /// upper bound; `frameLength` is updated to the actual decoded count.
    func decode(_ packet: UnsafeBufferPointer<UInt8>, to buffer: AVAudioPCMBuffer) throws {
        guard let base = packet.baseAddress, !packet.isEmpty else { return }
        let byteCount = packet.count

        let compressed = AVAudioCompressedBuffer(
            format: self.inputFormat,
            packetCapacity: 1,
            maximumPacketSize: byteCount
        )
        memcpy(compressed.data, base, byteCount)
        compressed.byteLength = UInt32(byteCount)
        compressed.packetCount = 1
        compressed.packetDescriptions?.pointee = AudioStreamPacketDescription(
            mStartOffset: 0,
            mVariableFramesInPacket: 0,
            mDataByteSize: UInt32(byteCount)
        )

        final class Fed: @unchecked Sendable { var done = false }
        let fed = Fed()
        var convErr: NSError?
        let status = self.converter.convert(to: buffer, error: &convErr) { _, statusPtr in
            if fed.done {
                statusPtr.pointee = .noDataNow
                return nil
            }
            fed.done = true
            statusPtr.pointee = .haveData
            return compressed
        }
        if status == .error {
            throw DecodeError.decodeFailed(convErr)
        }
    }
}
