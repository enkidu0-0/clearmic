package dev.sonivane.engine

import android.net.Uri

/** What the PC's QR code carries: sonivane://<host>:<port>?key=<64 hex>&name=<pc name> */
data class PairedPc(val name: String, val host: String, val port: Int, val keyHex: String) {
    companion object {
        fun parse(text: String): PairedPc? {
            val u = runCatching { Uri.parse(text.trim()) }.getOrNull() ?: return null
            if (u.scheme != "sonivane") return null
            val host = u.host ?: return null
            val port = if (u.port > 0) u.port else 47100
            val key = u.getQueryParameter("key") ?: return null
            if (Protocol.hexToKey(key) == null) return null
            return PairedPc(u.getQueryParameter("name") ?: host, host, port, key.lowercase())
        }
    }
}
