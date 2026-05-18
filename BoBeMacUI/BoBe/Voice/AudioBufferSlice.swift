@preconcurrency import AVFoundation

/// Returns a new `AVAudioPCMBuffer` containing `frames` samples starting
/// at `offset` in `source`. Returns `nil` on allocation failure or if
/// the source buffer has no `int16ChannelData` (mismatched format).
///
/// Standalone helper — used by `VoicePipeline.truncatePlayback` to slice
/// the head of a straddling chunk after barge-in. No instance state needed.
func sliceInt16Buffer(
    _ source: AVAudioPCMBuffer,
    offset: AVAudioFrameCount,
    frames: AVAudioFrameCount
) -> AVAudioPCMBuffer? {
    guard frames > 0,
          offset + frames <= source.frameLength,
          let dst = AVAudioPCMBuffer(pcmFormat: source.format, frameCapacity: frames),
          let srcCh = source.int16ChannelData?[0],
          let dstCh = dst.int16ChannelData?[0]
    else {
        return nil
    }
    dst.frameLength = frames
    dstCh.update(from: srcCh.advanced(by: Int(offset)), count: Int(frames))
    return dst
}
