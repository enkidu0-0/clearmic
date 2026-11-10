package dev.sonivane.engine

import android.app.Application
import android.content.Intent
import androidx.core.content.ContextCompat
import androidx.lifecycle.AndroidViewModel
import dev.sonivane.service.MicStreamService

class MicViewModel(app: Application) : AndroidViewModel(app) {
    val state = EngineHolder.state

    /** The caller has RECORD_AUDIO. The service moves to the foreground and starts the engine. */
    fun start() {
        val app = getApplication<Application>()
        ContextCompat.startForegroundService(app, Intent(app, MicStreamService::class.java))
    }

    fun stop() {
        val app = getApplication<Application>()
        app.stopService(Intent(app, MicStreamService::class.java))
        EngineHolder.stopEngine()
    }

    fun pair(pc: PairedPc) = EngineHolder.pair(pc)
    fun setMuted(v: Boolean) = EngineHolder.setMuted(v)
    fun setGain(db: Float) = EngineHolder.setGain(db)
    fun setBitrate(kbps: Int) = EngineHolder.setBitrate(kbps)
}
