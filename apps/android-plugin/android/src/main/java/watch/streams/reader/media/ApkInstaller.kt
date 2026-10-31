// In-app self-update (sideloaded APK). The Rust updater downloads the APK from
// https://listen.streams.watch/updates/android/<channel>/latest.json, checks size, SHA-256 and the minisign
// signature, then calls installApk. Android itself refuses an update signed with a different certificate.
//
// Rules (developer.android.com PackageInstaller.SessionParams, verified 2026-10-30):
// - REQUEST_INSTALL_PACKAGES + the per-app "Install unknown apps" toggle, granted by the user once.
// - setRequireUserAction(USER_ACTION_NOT_REQUIRED) (API 31+) installs without a prompt when the app updates itself,
//   holds UPDATE_PACKAGES_WITHOUT_USER_ACTION, and the new APK targets a recent SDK (>= 34 on Android 16, >= 35 on
//   Android 17). Otherwise the session returns STATUS_PENDING_USER_ACTION and we show the system dialog.
// - Developer verification (enforced from 2026-09-30 in BR, ID, SG, TH; global 2027): updates of unregistered apps
//   fail on certified devices unless the user enabled the advanced flow. Register watch.streams.reader first.
package watch.streams.reader.media

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.pm.PackageInstaller
import android.os.Build
import app.tauri.plugin.JSObject
import java.io.File

class ApkInstaller(private val activity: Activity) {
    fun install(path: String, onResult: (JSObject) -> Unit) {
        val ctx = activity.applicationContext
        if (!ctx.packageManager.canRequestPackageInstalls()) {
            // First update: send the user to "Install unknown apps" for this app, then retry.
            onResult(updateEvent("needsUserAction", "Allow Streams Reader to install updates, then try again."))
            activity.startActivity(
                Intent(android.provider.Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
                    android.net.Uri.parse("package:${ctx.packageName}")),
            )
            return
        }
        val installer = ctx.packageManager.packageInstaller
        val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL).apply {
            setAppPackageName(ctx.packageName)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
                setRequireUserAction(PackageInstaller.SessionParams.USER_ACTION_NOT_REQUIRED)
            }
        }
        val apk = File(path)
        val id = installer.createSession(params)
        installer.openSession(id).use { session ->
            session.openWrite("base.apk", 0, apk.length()).use { out ->
                apk.inputStream().use { it.copyTo(out) }
                session.fsync(out)
            }
            val intent = Intent(ctx, InstallResultReceiver::class.java)
            val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
            session.commit(PendingIntent.getBroadcast(ctx, id, intent, flags).intentSender)
        }
        // The process is killed and restarted by the system on success; the receiver handles the other outcomes.
    }
}

class InstallResultReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        when (intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)) {
            PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                // Not every update qualifies for a silent install: show the system confirmation.
                @Suppress("DEPRECATION")
                val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
                confirm?.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)?.let(context::startActivity)
            }
            PackageInstaller.STATUS_SUCCESS -> Unit
            else -> {
                // TODO(phase 3): report EXTRA_STATUS_MESSAGE as UpdateEvent.failed through the plugin channel.
            }
        }
    }
}

internal fun updateEvent(kind: String, message: String): JSObject =
    JSObject().put("kind", kind).put("message", message)
