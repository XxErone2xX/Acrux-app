package com.acrux.next

import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import android.provider.DocumentsContract
import android.provider.Settings
import androidx.core.content.FileProvider
import app.tauri.annotation.InvokeArg
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.time.Instant

@InvokeArg
class ServiceOptions { var action = ""; var payload = "{}" }

/** User-facing management actions. Game input and OCR never pass through this service. */
object MobileServices {
    private val lock = Any()
    private var update = JSONObject().put("busy", false).put("available", false).put("ready", false).put("message", "확인 전")
    private val hosts = setOf("github.com", "api.github.com", "objects.githubusercontent.com", "release-assets.githubusercontent.com")
    private val websites = setOf("https://discord.gg/cyK26gsjCC", "https://www.roblox.com/ko/users/11743130211/profile", "https://ko-fi.com/acrux_", "https://acruxmacro.com")

    private fun state() = synchronized(lock) { JSObject(update.toString()) }
    private fun change(block: (JSONObject) -> Unit) = synchronized(lock) { block(update) }
    private fun hash(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().use { input ->
            val buffer = ByteArray(65536)
            while (true) { val count = input.read(buffer); if (count < 0) break; digest.update(buffer, 0, count) }
        }
        return digest.digest().joinToString("") { "%02x".format(it.toInt() and 255) }
    }
    private fun open(url: String): HttpURLConnection {
        var next = url
        repeat(6) {
            val parsed = URL(next)
            require(parsed.protocol == "https" && parsed.host in hosts && parsed.userInfo == null) { "공식 배포 주소가 아닙니다." }
            val connection = parsed.openConnection() as HttpURLConnection
            connection.connectTimeout = 10000
            connection.readTimeout = 30000
            connection.instanceFollowRedirects = false
            connection.setRequestProperty("User-Agent", "Acrux-Dev-Updater")
            val status = connection.responseCode
            if (status in 300..399) {
                next = connection.getHeaderField("Location") ?: run { connection.disconnect(); error("다운로드 주소가 없습니다.") }
                connection.disconnect()
            } else {
                if (status !in 200..299) { connection.disconnect(); error("업데이트 다운로드 HTTP $status") }
                return connection
            }
        }
        error("다운로드 리디렉션이 너무 많습니다.")
    }
    @Suppress("DEPRECATION")
    private fun validatePackage(activity: Activity, file: File) {
        val manager = activity.packageManager
        val downloaded = manager.getPackageArchiveInfo(file.path, PackageManager.GET_SIGNATURES) ?: error("APK 파일을 읽지 못했습니다.")
        val installed = manager.getPackageInfo(activity.packageName, PackageManager.GET_SIGNATURES)
        require(downloaded.packageName == activity.packageName) { "다른 앱의 APK입니다. 정식 앱과 개발 앱은 별도로 업데이트합니다." }
        val installedVersion = if (Build.VERSION.SDK_INT >= 28) installed.longVersionCode else installed.versionCode.toLong()
        val downloadedVersion = if (Build.VERSION.SDK_INT >= 28) downloaded.longVersionCode else downloaded.versionCode.toLong()
        require(downloadedVersion >= installedVersion) { "설치된 앱보다 오래된 APK입니다." }
        val signatures = installed.signatures?.map { it.toCharsString() }?.toSet().orEmpty()
        require(signatures.isNotEmpty() && signatures == downloaded.signatures?.map { it.toCharsString() }?.toSet()) { "설치된 앱과 업데이트 서명이 다릅니다." }
    }
    private fun ui(activity: Activity, invoke: Invoke, block: () -> JSONObject) {
        activity.runOnUiThread {
            try { invoke.resolve(JSObject(block().toString())) }
            catch (error: Exception) { invoke.reject(error.message ?: "작업을 완료하지 못했습니다.") }
        }
    }
    fun action(activity: Activity, invoke: Invoke) {
        val options = invoke.parseArgs(ServiceOptions::class.java)
        val payload = JSONObject(options.payload)
        when (options.action) {
            "openWeb" -> ui(activity, invoke) {
                val url = payload.getString("url")
                require(url in websites) { "허용되지 않은 지원 링크입니다." }
                activity.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                JSONObject().put("ok", true)
            }
            "openFolder" -> ui(activity, invoke) {
                val folder = File(payload.getString("path")).canonicalFile
                require(folder.toPath().startsWith(activity.dataDir.canonicalFile.toPath())) { "앱 데이터 폴더만 열 수 있습니다." }
                activity.getSharedPreferences("support", 0).edit().putString("documentsRoot", folder.path).apply()
                val uri = DocumentsContract.buildDocumentUri(activity.packageName + ".documents", "root")
                val intent = Intent(Intent.ACTION_VIEW).setDataAndType(uri, DocumentsContract.Document.MIME_TYPE_DIR).addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                if (intent.resolveActivity(activity.packageManager) != null) activity.startActivity(intent)
                else activity.startActivity(Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("*/*").putExtra(DocumentsContract.EXTRA_INITIAL_URI, uri))
                JSONObject().put("ok", true)
            }
            "restart" -> ui(activity, invoke) {
                Native.control(0)
                activity.stopService(Intent(activity, CaptureService::class.java))
                val launch = activity.packageManager.getLaunchIntentForPackage(activity.packageName) ?: error("앱 시작 화면이 없습니다.")
                launch.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK)
                activity.startActivity(launch)
                activity.finish()
                JSONObject().put("ok", true)
            }
            "uninstall" -> ui(activity, invoke) {
                Native.control(0)
                activity.startActivity(Intent(Intent.ACTION_DELETE, Uri.parse("package:" + activity.packageName)))
                JSONObject().put("ok", true)
            }
            "updateState" -> invoke.resolve(state())
            "updateCheck" -> Thread({
                try {
                    val installed = activity.packageManager.getPackageInfo(activity.packageName, 0)
                    val actual = hash(File(activity.applicationInfo.sourceDir))
                    val changedAt = runCatching { Instant.parse(payload.optString("updatedAt")).toEpochMilli() }.getOrDefault(0)
                    val available = actual != payload.getString("sha256") && changedAt > installed.lastUpdateTime
                    change {
                        it.put("available", available).put("current", installed.versionName).put("version", payload.optString("version"))
                            .put("notes", payload.optString("notes")).put("sha256", payload.getString("sha256")).put("url", payload.getString("url"))
                            .put("message", if (available) "새 업데이트가 있습니다." else "최신 버전입니다.").remove("error")
                    }
                    invoke.resolve(state())
                } catch (error: Exception) { invoke.reject(error.message ?: "업데이트 확인 실패") }
            }, "acrux-update-check").start()
            "updateDownload" -> {
                synchronized(lock) {
                    if (update.optBoolean("busy")) { invoke.reject("업데이트 파일을 이미 받고 있습니다."); return }
                    update.put("busy", true).put("message", "업데이트 다운로드 중…").put("ready", false).remove("error")
                }
                Thread({
                    val part = File(activity.cacheDir, "acrux-update.part")
                    try {
                        val wanted = payload.getString("sha256").lowercase()
                        require(Regex("[0-9a-f]{64}").matches(wanted)) { "업데이트 SHA256이 올바르지 않습니다." }
                        val connection = open(payload.getString("url"))
                        try {
                            connection.inputStream.use { input -> part.outputStream().use { output ->
                                val buffer = ByteArray(65536); var total = 0L
                                while (true) {
                                    val size = input.read(buffer); if (size < 0) break
                                    total += size; require(total <= 512L * 1024 * 1024) { "업데이트 파일이 너무 큽니다." }
                                    output.write(buffer, 0, size)
                                    change { it.put("received", total).put("size", connection.contentLengthLong) }
                                }
                            } }
                        } finally { connection.disconnect() }
                        require(hash(part) == wanted) { "업데이트 파일 SHA256이 일치하지 않습니다." }
                        validatePackage(activity, part)
                        val target = File(activity.cacheDir, "acrux-update.apk")
                        if (target.exists()) require(target.delete()) { "이전 업데이트 파일을 지우지 못했습니다." }
                        require(part.renameTo(target)) { "업데이트 파일을 저장하지 못했습니다." }
                        change { it.put("ready", true).put("available", true).put("sha256", wanted).put("message", "설치 준비 완료") }
                        invoke.resolve(state())
                    } catch (error: Exception) {
                        part.delete()
                        change { it.put("error", error.message ?: "업데이트 다운로드 실패").put("message", "업데이트 다운로드 실패") }
                        invoke.reject(error.message ?: "업데이트 다운로드 실패")
                    } finally { change { it.put("busy", false) } }
                }, "acrux-update-download").start()
            }
            "updateInstall" -> ui(activity, invoke) {
                val file = File(activity.cacheDir, "acrux-update.apk")
                require(file.isFile && state().optBoolean("ready")) { "받은 업데이트 파일이 없습니다." }
                require(hash(file) == state().getString("sha256")) { "업데이트 파일 검증이 실패했습니다." }
                validatePackage(activity, file)
                if (!activity.packageManager.canRequestPackageInstalls()) {
                    activity.startActivity(Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:" + activity.packageName)))
                    JSONObject().put("permissionRequired", true).put("message", "이 앱의 업데이트 설치를 허용한 뒤 다시 설치를 누르세요.")
                } else {
                    val uri = FileProvider.getUriForFile(activity, activity.packageName + ".fileprovider", file)
                    activity.startActivity(Intent(Intent.ACTION_VIEW).setDataAndType(uri, "application/vnd.android.package-archive").addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION))
                    JSONObject().put("ok", true).put("message", "설치 화면에서 업데이트를 확인하세요.")
                }
            }
            else -> invoke.reject("알 수 없는 지원 작업입니다.")
        }
    }
}
