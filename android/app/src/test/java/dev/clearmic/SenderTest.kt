package dev.clearmic

import dev.clearmic.engine.Packetizer
import dev.clearmic.engine.Protocol
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.net.DatagramPacket
import java.net.DatagramSocket
import java.net.InetAddress
import java.nio.ByteBuffer
import java.nio.ByteOrder

class SenderTest {
    private val key = ByteArray(32) { 0xab.toByte() }

    @Test fun datagram_layout_matches_the_pc_protocol() {
        val pcm = ShortArray(Protocol.FRAME_SAMPLES) { (3000 * Math.sin(it / 5.0)).toInt().toShort() }
        val d = Packetizer(key, startSeq = 7).next(pcm)
        assertEquals(0xC1.toByte(), d[0]); assertEquals(0xEA.toByte(), d[1]); assertEquals(1.toByte(), d[2])
        val b = ByteBuffer.wrap(d).order(ByteOrder.LITTLE_ENDIAN)
        assertEquals(7, b.getInt(4))
        assertEquals(d.size - Protocol.HEADER_LEN, b.getShort(16).toInt() and 0xFFFF)
    }

    @Test fun pairing_key_parsing() {
        assertTrue(Protocol.hexToKey("ab".repeat(32))!!.contentEquals(key))
        assertEquals(null, Protocol.hexToKey("zz"))
    }

    /** Sends a wav through the real Opus + encryption path to a PC receiver on 127.0.0.1:47100. */
    @Test fun send_wav_to_pc_receiver() {
        val path = System.getenv("CLEARMIC_TEST_WAV") ?: return // only when the PC receiver is running
        val raw = File(path).readBytes()
        val pcm = ByteBuffer.wrap(raw, 44, raw.size - 44).order(ByteOrder.LITTLE_ENDIAN).asShortBuffer()
        val p = Packetizer(key)
        val sock = DatagramSocket()
        val addr = InetAddress.getByName("127.0.0.1")
        val frame = ShortArray(Protocol.FRAME_SAMPLES)
        val t0 = System.nanoTime()
        var n = 0
        while (pcm.remaining() >= frame.size) {
            pcm.get(frame)
            val d = p.next(frame)
            sock.send(DatagramPacket(d, d.size, addr, 47100))
            n++
            val due = t0 + n * 10_000_000L
            while (System.nanoTime() < due) Thread.sleep(1)
        }
        println("sent $n packets")
        assertTrue(n > 100)
    }
}
