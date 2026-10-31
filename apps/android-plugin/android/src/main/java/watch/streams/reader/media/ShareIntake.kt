// Share sheet and "open with": turns ACTION_SEND / ACTION_VIEW intents into OpenRequest JSON (tag "kind": file /
// url / text, see streams-native-bridge proto). Content URIs are copied into <files>/imports/ right away because
// the read grant ends with the activity.
package watch.streams.reader.media

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import app.tauri.plugin.JSArray
import app.tauri.plugin.JSObject
import java.io.File

class ShareIntake(private val context: Context) {
    private val pending = mutableListOf<JSObject>()

    fun accept(intent: Intent?) {
        intent ?: return
        when (intent.action) {
            Intent.ACTION_SEND -> {
                @Suppress("DEPRECATION")
                val stream = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
                val text = intent.getStringExtra(Intent.EXTRA_TEXT)
                when {
                    stream != null -> copy(stream, intent.type)?.let(pending::add)
                    text != null && looksLikeUrl(text) -> pending.add(JSObject().put("kind", "url").put("url", text.trim()))
                    text != null -> pending.add(
                        JSObject().put("kind", "text").put("text", text)
                            .put("subject", intent.getStringExtra(Intent.EXTRA_SUBJECT)),
                    )
                }
            }
            Intent.ACTION_VIEW -> intent.data?.takeIf { it.scheme == "content" || it.scheme == "file" }
                ?.let { copy(it, intent.type) }?.let(pending::add)
        }
    }

    fun drain(): JSArray {
        val out = JSArray()
        pending.forEach { out.put(it) }
        pending.clear()
        return out
    }

    private fun looksLikeUrl(s: String) = s.trim().let { (it.startsWith("https://") || it.startsWith("http://")) && !it.contains(' ') }

    private fun copy(uri: Uri, mime: String?): JSObject? = runCatching {
        val name = displayName(uri) ?: "shared"
        val dir = File(context.filesDir, "imports").apply { mkdirs() }
        val target = File(dir, "${System.currentTimeMillis()}-${name.replace('/', '_')}")
        // TODO(phase 4): refuse files above the import size limit (same limits as crates/import) before copying.
        context.contentResolver.openInputStream(uri)?.use { input -> target.outputStream().use { input.copyTo(it) } }
        JSObject().put("kind", "file").put("path", target.absolutePath).put("name", name)
            .put("mime", mime ?: context.contentResolver.getType(uri)).put("size", target.length())
    }.getOrNull()

    private fun displayName(uri: Uri): String? =
        context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
            if (c.moveToFirst()) c.getString(0) else null
        } ?: uri.lastPathSegment
}
