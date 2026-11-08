package dev.clearmic.service

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import dev.clearmic.MainActivity
import dev.clearmic.engine.EngineHolder

/** Keeps the mic running with the screen off. Android requires the notification. */
class MicStreamService : Service() {
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "Microphone streaming", NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        val n = Notification.Builder(this, CHANNEL)
            .setContentTitle("ClearMic is sending your voice")
            .setContentText("Tap to open")
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentIntent(open)
            .setOngoing(true)
            .build()
        startForeground(1, n, ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE)
        EngineHolder.startEngine(applicationContext)
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        EngineHolder.stopEngine()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    private companion object { const val CHANNEL = "stream" }
}
