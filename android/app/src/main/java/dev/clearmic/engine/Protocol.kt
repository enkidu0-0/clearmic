package dev.clearmic.engine

import java.nio.ByteBuffer
import java.nio.ByteOrder
import javax.crypto.Cipher
import javax.crypto.spec.IvParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Wire format shared with the PC (desktop/crates/protocol, ARCHITECTURE.md section 4).
 * Datagram = 18-byte header + ChaCha20-Poly1305 (payload + 16-byte tag).
 * Plain JVM code, no Android imports, so it is unit-tested on the host.
 */
object Protocol {
    const val HEADER_LEN = 18
    const val FRAME_SAMPLES = 480 // 10 ms at 48 kHz
    const val SAMPLE_RATE = 48_000
    const val FLAG_MUTED = 1
    const val FLAG_PHONE_DENOISE = 2
    const val FLAG_RAW16 = 8
    private const val VERSION = 1
    private const val TAG_LEN = 16

    fun hexToKey(hex: String): ByteArray? {
        val h = hex.trim()
        if (h.length != 64 || !h.all { it.isDigit() || it.lowercaseChar() in 'a'..'f' }) return null
        return ByteArray(32) { h.substring(it * 2, it * 2 + 2).toInt(16).toByte() }
    }

    /**
     * One datagram. The nonce is seq | ts_us, so [tsUs] must be unique per key across sessions:
     * callers pass wall-clock microseconds, never time since the stream started.
     */
    fun seal(cipher: Cipher, key: ByteArray, flags: Int, seq: Int, tsUs: Long, payload: ByteArray, len: Int = payload.size): ByteArray {
        val header = ByteBuffer.allocate(HEADER_LEN).order(ByteOrder.LITTLE_ENDIAN)
            .put(0xC1.toByte()).put(0xEA.toByte()).put(VERSION.toByte()).put(flags.toByte())
            .putInt(seq).putLong(tsUs).putShort((len + TAG_LEN).toShort()).array()
        val nonce = ByteBuffer.allocate(12).order(ByteOrder.LITTLE_ENDIAN).putInt(seq).putLong(tsUs).array()
        cipher.init(Cipher.ENCRYPT_MODE, SecretKeySpec(key, "ChaCha20"), IvParameterSpec(nonce))
        cipher.updateAAD(header)
        val ct = cipher.doFinal(payload, 0, len)
        return header + ct
    }

    fun newCipher(): Cipher = Cipher.getInstance("ChaCha20-Poly1305")
}
