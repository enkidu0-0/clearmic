package dev.sonivane.engine

import io.github.jaredmdobson.concentus.OpusApplication
import io.github.jaredmdobson.concentus.OpusEncoder
import io.github.jaredmdobson.concentus.OpusSignal

/** PCM frames in, encrypted Opus datagrams out. No Android types, so it runs in unit tests. */
class Packetizer(private val key: ByteArray, bitrateKbps: Int = 64, startSeq: Int = (Math.random() * Int.MAX_VALUE).toInt()) {
    private val enc = OpusEncoder(Protocol.SAMPLE_RATE, 1, OpusApplication.OPUS_APPLICATION_VOIP).apply {
        bitrate = bitrateKbps * 1000
        complexity = 5
        signalType = OpusSignal.OPUS_SIGNAL_VOICE
        useVBR = true
        useInbandFEC = false
        packetLossPercent = 5
    }
    private val cipher = Protocol.newCipher()
    private val out = ByteArray(1275)
    private var seq = startSeq
    private val sessionStartUs = System.currentTimeMillis() * 1000L

    fun setBitrate(kbps: Int) { enc.bitrate = kbps * 1000 }

    /** [pcm] holds exactly 480 samples. */
    fun next(pcm: ShortArray, flags: Int = 0): ByteArray {
        val n = enc.encode(pcm, 0, Protocol.FRAME_SAMPLES, out, 0, out.size)
        val s = seq++
        val ts = sessionStartUs + (s.toLong() and 0xFFFFFFFFL) * 10_000L
        return Protocol.seal(cipher, key, flags, s, ts, out, n)
    }
}
