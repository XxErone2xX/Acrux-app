package com.acrux.next

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.os.IBinder
import android.provider.Settings
import org.json.JSONObject
import java.util.concurrent.atomic.AtomicBoolean

/** Android-only log transport. Biome state and outgoing alerts are owned by Rust. */
class LogService : Service() {
    @Volatile private var worker: Thread? = null
    @Volatile private var files: Thread? = null
    @Volatile private var testing = false
    @Volatile private var resumeAt: String? = null
    @Volatile private var lastLineAt = 0L
    private val timestamp = Regex("^\\d{2}-\\d{2} \\d{2}:\\d{2}:\\d{2}\\.\\d{3}")

    override fun onBind(intent: Intent?): IBinder? = null
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val enabled = JSONObject(Native.biomeStatus()).getJSONObject("settings").optBoolean("enabled")
        val test = intent?.action == TEST
        if (!enabled && !test) { stopSelf(); return START_NOT_STICKY }
        if (worker?.isAlive == true && (!test || testing)) return START_NOT_STICKY
        worker?.interrupt()
        files?.interrupt()
        testing = test
        notice(if (test) "바이옴 로그 테스트 중…" else "바이옴 로그 감시 중")
        worker = Thread({ if (test) testLogs() else watch() }, "acrux-biome-log").also { it.start() }
        return START_NOT_STICKY
    }

    private fun current() = worker === Thread.currentThread() && !Thread.currentThread().isInterrupted
    private fun waitFor(ms: Long): Boolean = try { Thread.sleep(ms); current() } catch (_: InterruptedException) { false }
    private fun watch() {
        val owner = Thread.currentThread()
        files = Thread({
            while (worker === owner && !Thread.currentThread().isInterrupted) {
                if (Adb.connected) {
                    try {
                        Adb.shell(this, FILES, 30 * 60 * 1000L) { line ->
                            if (worker === owner && !Thread.currentThread().isInterrupted) Native.biomeLine(line, 2, false)
                        }
                    } catch (_: Exception) { }
                }
                try { Thread.sleep(5000) } catch (_: InterruptedException) { break }
            }
        }, "acrux-biome-files").also { it.start() }
        try {
            while (current()) {
                val connection = Adb.connect(this)
                adbState = connection
                if (!current()) return
                if (connection != "ok") {
                    Native.biomeConnection(false, if (connection == "pair") "무선 디버깅 페어링 필요" else "무선 디버깅 연결 안 됨 — 다시 시도 중")
                    if (!waitFor(10000)) return
                    continue
                }
                var from = resumeAt
                if (from == null || System.currentTimeMillis() - lastLineAt > 300000) {
                    var last: String? = null
                    try {
                        Adb.shell(this, SNAPSHOT, 20000) { line ->
                            val time = timestamp.find(line)?.value
                            if (time != null) from = time
                            else if (line.contains("BloxstrapRPC") || line.contains("Client:Disconnect")) last = line
                        }
                        if (current()) last?.let { Native.biomeLine(it, 1, true) }
                    } catch (_: Exception) { }
                }
                if (!current()) return
                Native.biomeConnection(true, "감시 중")
                try {
                    Adb.shell(this, "logcat -v time -T " + (from?.let { "'$it'" } ?: "1"), 30 * 60 * 1000L, 120000) { line ->
                        if (worker === owner && !Thread.currentThread().isInterrupted) {
                            timestamp.find(line)?.value?.let { resumeAt = it }
                            lastLineAt = System.currentTimeMillis()
                            Native.biomeLine(line, 1, false)
                        }
                    }
                } catch (_: Exception) { }
                if (current()) Native.biomeConnection(false, "바이옴 로그 다시 연결 중")
                if (!waitFor(1000)) return
            }
        } finally {
            if (worker === owner) files?.interrupt()
        }
    }

    private fun testLogs() {
        val owner = Thread.currentThread()
        testOutput = "무선 디버깅 연결 중…"
        testUntil = System.currentTimeMillis() + 90000
        try {
            adbState = Adb.connect(this)
            if (!current()) return
            if (adbState != "ok") { testOutput = "무선 디버깅 연결을 확인하세요."; return }
            val found = java.util.ArrayDeque<String>()
            fun append(line: String) {
                if (worker !== owner || Thread.currentThread().isInterrupted) return
                if (line.contains("BloxstrapRPC") || line.contains("Client:Disconnect")) {
                    synchronized(found) {
                        found.addLast(line.take(2000))
                        while (found.size > 30) found.removeFirst()
                        testOutput = found.joinToString("\n")
                    }
                }
            }
            testOutput = "쌓인 로그 · 로그 파일 확인 중…"
            Adb.shell(this, "logcat -d -v brief | grep -a -e BloxstrapRPC -e Client:Disconnect | tail -n 20", 15000, ::append)
            Adb.shell(this, "find /sdcard/Android/data/com.roblox.client -iname '*.log' -exec tail -n 40 {} \\; 2>/dev/null", 15000, ::append)
            testUntil = System.currentTimeMillis() + 90000
            if (found.isEmpty()) testOutput = "새 로그 확인 중… 로블록스에서 바이옴이 바뀔 때까지 기다리세요."
            Adb.shell(this, "logcat -v brief -T 1", 90000, ::append)
            if (current() && found.isEmpty()) testOutput = "바이옴 로그를 찾지 못했습니다. 로블록스 실행 상태를 확인하세요."
        } catch (_: Exception) {
            if (current()) testOutput = "바이옴 로그 테스트 연결이 끊겼습니다."
        } finally {
            if (worker === owner) {
                testUntil = 0
                Native.platformLog(testOutput, 'c')
                testing = false
                Adb.notifyDone(this, "바이옴 로그 테스트 완료 — 앱에서 결과를 확인하세요.")
                worker = null
                if (JSONObject(Native.biomeStatus()).getJSONObject("settings").optBoolean("enabled")) refresh(this)
                else stopSelf()
            }
        }
    }

    private fun notice(message: String) {
        val nm = getSystemService(NotificationManager::class.java)
        nm.createNotificationChannel(NotificationChannel(CHANNEL, "바이옴 감시", NotificationManager.IMPORTANCE_LOW))
        val pending = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE)
        startForeground(7303, Notification.Builder(this, CHANNEL)
            .setSmallIcon(android.R.drawable.stat_notify_sync).setContentTitle("Acrux Mobile")
            .setContentText(message).setContentIntent(pending).setOngoing(true).build())
    }
    override fun onDestroy() {
        worker?.interrupt()
        files?.interrupt()
        worker = null
        testUntil = 0
        super.onDestroy()
    }
    companion object {
        private const val CHANNEL = "acrux_biome"
        private const val TEST = "com.acrux.next.BIOME_TEST"
        private const val SNAPSHOT = "logcat -d -v time 2>/dev/null | tail -n 1; logcat -d -v brief 2>/dev/null | grep -a -e BloxstrapRPC -e Client:Disconnect | tail -n 3"
        private const val FILES = "p=; f=; s=0; trap '[ -n \"\$p\" ] && kill \$p 2>/dev/null' EXIT; while :; do l=\$(find /sdcard/Android/data/com.roblox.client -iname '*.log' 2>/dev/null); n=; [ -n \"\$l\" ] && n=\$(ls -t \$l 2>/dev/null | head -n 1); if [ -n \"\$n\" ] && [ \"\$n\" != \"\$f\" ]; then [ -n \"\$p\" ] && kill \$p 2>/dev/null; f=\"\$n\"; if [ \$s = 0 ]; then tail -n 0 -f \"\$f\" & else tail -n +1 -f \"\$f\" & fi; p=\$!; s=1; fi; sleep 5; done"
        @Volatile var adbState = "off"
        @Volatile var testOutput = ""
        @Volatile var testUntil = 0L
        private val connecting = AtomicBoolean(false)

        @JvmStatic fun refresh(context: Context) {
            val enabled = JSONObject(Native.biomeStatus()).getJSONObject("settings").optBoolean("enabled")
            if (enabled) context.startForegroundService(Intent(context, LogService::class.java))
            else if (testUntil == 0L) context.stopService(Intent(context, LogService::class.java))
        }
        @JvmStatic fun tr(context: Context, text: String): String {
            val lang = context.getSharedPreferences("overlay", Context.MODE_PRIVATE).getString("lang", "ko")
            val column = when (lang) { "en" -> 1; "ja" -> 2; else -> return text }
            return context.assets.open("i18n/mobile.tsv").bufferedReader().useLines { lines ->
                lines.map { it.split('\t') }.firstOrNull { it[0] == text }?.getOrNull(column)?.replace("\\n", "\n") ?: text
            }
        }
        fun action(context: Context, action: String) {
            when (action) {
                "pair" -> {
                    if (!Adb.supported()) throw IllegalStateException("무선 디버깅은 안드로이드 11 이상에서 사용할 수 있습니다.")
                    Adb.showPairNotice(context, "무선 디버깅 → '페어링 코드로 기기 페어링' 창을 띄운 채로 [코드 입력] 에 6자리 코드")
                    context.startActivity(Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                }
                "developer" -> context.startActivity(Intent(Settings.ACTION_APPLICATION_DEVELOPMENT_SETTINGS).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                "connect" -> if (connecting.compareAndSet(false, true)) {
                    Thread {
                        try { adbState = Adb.connect(context); refresh(context) }
                        finally { connecting.set(false) }
                    }.start()
                }
                "testLogs" -> context.startForegroundService(Intent(context, LogService::class.java).setAction(TEST))
                "refresh" -> refresh(context)
                else -> throw IllegalArgumentException("알 수 없는 무선 디버깅 작업입니다.")
            }
        }
    }
}
