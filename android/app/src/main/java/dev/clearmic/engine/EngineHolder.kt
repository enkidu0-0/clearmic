package dev.clearmic.engine

import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import dev.clearmic.service.MicStreamService
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/** One engine for the whole app. The foreground service starts it, the UI reads its state. */
object EngineHolder {
    private val _state = MutableStateFlow(MicState())
    val state: StateFlow<MicState> = _state.asStateFlow()

    private var engine: AudioEngine? = null
    private var prefs: SharedPreferences? = null
    private var appContext: Context? = null
    private val ring = ArrayDeque<Float>()
    private var frame = 0

    fun init(context: Context) {
        if (prefs != null) return
        appContext = context.applicationContext
        val p = context.getSharedPreferences("clearmic", Context.MODE_PRIVATE).also { prefs = it }
        val paired = p.getString("key", null)?.let { key ->
            PairedPc(p.getString("name", "my pc")!!, p.getString("host", "")!!, p.getInt("port", 47100), key)
        }
        _state.update { it.copy(paired = paired, bitrateKbps = p.getInt("bitrate", 64)) }
    }

    fun pair(pc: PairedPc) {
        prefs?.edit()?.putString("name", pc.name)?.putString("host", pc.host)?.putInt("port", pc.port)?.putString("key", pc.keyHex)?.apply()
        _state.update { it.copy(paired = pc) }
    }

    /** Called by the service once it is in the foreground. */
    fun startEngine(context: Context) {
        val pc = _state.value.paired ?: return
        stopEngine()
        ring.clear()
        _state.update { it.copy(link = Link.Connecting, levels = emptyList()) }
        engine = AudioEngine(
            context.applicationContext, pc,
            settings = { _state.value.let { AudioEngine.Settings(it.muted, it.gainDb, it.bitrateKbps) } },
            onLevel = ::pushLevel,
            onFailure = { reason ->
                engine = null
                _state.update { it.copy(link = Link.Failed(reason)) }
                appContext?.let { c -> c.stopService(Intent(c, MicStreamService::class.java)) }
            },
        ).also { it.start() }
        // UDP has no handshake yet, so "live" means "sending".
        _state.update { it.copy(link = Link.Live(pc.name, "wi-fi")) }
    }

    fun stopEngine() {
        engine?.stop()
        engine = null
        _state.update { if (it.link is Link.Failed) it else it.copy(link = Link.Idle, levels = emptyList()) }
    }

    private fun pushLevel(v: Float) {
        ring.addLast(v)
        if (ring.size > 300) ring.removeFirst()
        if (++frame % 3 == 0) _state.update { it.copy(levels = ring.toList()) } // about 33 updates a second
    }

    fun setMuted(v: Boolean) = _state.update { it.copy(muted = v) }
    fun setGain(db: Float) = _state.update { it.copy(gainDb = db) }
    fun setBitrate(kbps: Int) {
        prefs?.edit()?.putInt("bitrate", kbps)?.apply()
        _state.update { it.copy(bitrateKbps = kbps) }
    }
}
