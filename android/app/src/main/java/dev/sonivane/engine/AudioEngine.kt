package dev.sonivane.engine

import android.annotation.SuppressLint
import android.content.Context
import android.media.AudioFormat
import android.media.AudioManager
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Process
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import kotlin.math.abs
import kotlin.math.pow

/**
 * Captures the mic (raw, no phone-side processing), encodes 10 ms Opus frames and sends them
 * encrypted over UDP. Runs on one high-priority thread. The noise removal happens on the PC.
 */
class AudioEngine(
    private val context: Context,
    private val pc: PairedPc,
    private val settings: () -> Settings,
    private val onLevel: (Float) -> Unit,
    private val onFailure: (String) -> Unit,
) {
    data class Settings(val muted: Boolean, val gainDb: Float, val bitrateKbps: Int)

    @Volatile private var running = false
    private var thread: Thread? = null

    fun start() {
        running = true
        thread = Thread({ run() }, "sonivane-audio").also { it.start() }
    }

    fun stop() {
        running = false
        thread?.join(1000)
        thread = null
    }

    @SuppressLint("MissingPermission") // the UI asks for RECORD_AUDIO before starting
    private fun run() {
        Process.setThreadPriority(Process.THREAD_PRIORITY_URGENT_AUDIO)
        val key = Protocol.hexToKey(pc.keyHex) ?: return onFailure("the pairing key is invalid, scan the code again")
        val am = context.getSystemService(Context.AUDIO_SERVICE) as AudioManager
        val unprocessed = am.getProperty(AudioManager.PROPERTY_SUPPORT_AUDIO_SOURCE_UNPROCESSED) == "true"
        val source = if (unprocessed) MediaRecorder.AudioSource.UNPROCESSED else MediaRecorder.AudioSource.VOICE_RECOGNITION
        val minBuf = AudioRecord.getMinBufferSize(Protocol.SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT)
        val rec = AudioRecord(
            source, Protocol.SAMPLE_RATE, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_16BIT,
            maxOf(minBuf, Protocol.FRAME_SAMPLES * 2 * 6),
        )
        if (rec.state != AudioRecord.STATE_INITIALIZED) return onFailure("the microphone could not be opened")

        val socket = try {
            DatagramSocket().apply { connect(InetAddress.getByName(pc.host), pc.port) }
        } catch (e: Exception) {
            rec.release(); return onFailure("cannot reach ${pc.host}: ${e.message}")
        }

        val packetizer = Packetizer(key, settings().bitrateKbps)
        var lastBitrate = settings().bitrateKbps
        val pcm = ShortArray(Protocol.FRAME_SAMPLES)
        var failedSends = 0
        try {
            rec.startRecording()
            while (running) {
                var got = 0
                while (got < pcm.size && running) {
                    val n = rec.read(pcm, got, pcm.size - got)
                    if (n < 0) { onFailure("the microphone stopped (error $n)"); return }
                    got += n
                }
                if (!running) break

                val s = settings()
                if (s.bitrateKbps != lastBitrate) { packetizer.setBitrate(s.bitrateKbps); lastBitrate = s.bitrateKbps }
                var flags = 0
                if (s.muted) {
                    pcm.fill(0); flags = Protocol.FLAG_MUTED
                } else if (s.gainDb != 0f) {
                    val g = 10f.pow(s.gainDb / 20f)
                    for (i in pcm.indices) pcm[i] = (pcm[i] * g).toInt().coerceIn(-32768, 32767).toShort()
                }
                var peak = 0
                for (v in pcm) peak = maxOf(peak, abs(v.toInt()))
                onLevel(peak / 32768f)

                // The PC app being restarted or Wi-Fi hiccuping makes a send fail for a moment ("connection refused").
                // Keep sending: it picks up again by itself. Give up only after about 8 seconds of nothing getting through.
                val d = packetizer.next(pcm, flags)
                try {
                    socket.send(DatagramPacket(d, d.size))
                    failedSends = 0
                } catch (e: java.io.IOException) {
                    if (++failedSends >= 800) { onFailure("lost the connection: ${e.message}"); return }
                }
            }
        } finally {
            runCatching { rec.stop() }
            rec.release()
            socket.close()
        }
    }
}
