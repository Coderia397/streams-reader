// Tauri plugin entry point. Commands are called by the app's Rust core (run_mobile_plugin), not by the webview.
// Transport goes to PlaybackService through a Media3 MediaController, so the service keeps playing when the
// activity (and this plugin instance) goes away; player events come back through the Rust-provided Channel.
package watch.streams.reader.media

import android.app.Activity
import android.content.ComponentName
import android.content.Intent
import android.webkit.WebView
import androidx.core.content.ContextCompat
import androidx.media3.common.Player
import androidx.media3.session.MediaController
import androidx.media3.session.SessionToken
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import com.google.common.util.concurrent.ListenableFuture

@TauriPlugin
class MediaPlugin(private val activity: Activity) : Plugin(activity) {
    private var controllerFuture: ListenableFuture<MediaController>? = null
    private var events: Channel? = null
    private val secrets by lazy { SecretStore(activity.applicationContext) }
    private val shares by lazy { ShareIntake(activity.applicationContext) }

    override fun load(webView: WebView) {
        // The intent that launched the activity may be a share or an "open with".
        shares.accept(activity.intent)
    }

    override fun onNewIntent(intent: Intent) {
        // TODO(phase 1): emit an `OpenRequest` through the event channel when the UI is already running.
        shares.accept(intent)
    }

    /** Connect (once) to PlaybackService; resolves on the main thread. */
    private fun withController(block: (MediaController) -> Unit) {
        val future = controllerFuture ?: MediaController.Builder(
            activity,
            SessionToken(activity, ComponentName(activity, PlaybackService::class.java)),
        ).buildAsync().also { f ->
            controllerFuture = f
            f.addListener({ runCatching { f.get() }.getOrNull()?.addListener(PlayerEvents { emit(it) }) },
                ContextCompat.getMainExecutor(activity))
        }
        future.addListener({ runCatching { future.get() }.getOrNull()?.let(block) },
            ContextCompat.getMainExecutor(activity))
    }

    /** Every event goes out as a `BridgeEvent` envelope: {"source": "player" | "update", "event": {...}}. */
    private fun emit(event: JSObject) {
        events?.send(event)
    }

    @Command
    fun setEventHandler(invoke: Invoke) {
        events = invoke.parseArgs(EventHandlerArgs::class.java).handler
        invoke.resolve()
    }

    /**
     * New document. Starting playback while the activity is visible gives the foreground service the
     * "while-in-use" capability Android 17 requires for apps targeting API 37 (notes/18).
     */
    @Command
    fun open(invoke: Invoke) {
        val args = invoke.parseArgs(OpenArgs::class.java)
        withController { c ->
            // TODO(phase 1): send OPEN as a custom SessionCommand carrying args (doc id, metadata, estimates) so
            // PlaybackService builds a fresh DocumentPlayer timeline; then seek to (startSegment, startMs).
            c.setPlaybackSpeed(args.rate.toFloat())
            if (args.autoplay) c.play()
            invoke.resolve()
        }
    }

    @Command
    fun append(invoke: Invoke) {
        val args = invoke.parseArgs(AppendArgs::class.java)
        withController { c ->
            // TODO(phase 1): custom command APPEND → PlaybackService.document.append(items); ExoPlayer plays
            // file:// WebM/Opus items back to back (gapless; see DocumentPlayer for the end-padding note).
            invoke.resolve()
        }
    }

    @Command
    fun control(invoke: Invoke) {
        val cmd = invoke.parseArgs(ControlArgs::class.java).command
        withController { c ->
            when (cmd.path("cmd").asText()) {
                "play" -> c.play()
                "pause" -> c.pause()
                "toggle" -> if (c.isPlaying) c.pause() else c.play()
                "seekBy" -> c.seekTo((c.currentPosition + cmd.path("deltaMs").asLong()).coerceAtLeast(0))
                "seekTo" -> {
                    // TODO(phase 1): map (segment, ms) to the DocumentPlayer's document-level position.
                }
                "next" -> c.seekToNextMediaItem()
                "previous" -> c.seekToPreviousMediaItem()
                "setRate" -> c.setPlaybackSpeed(cmd.path("rate").asDouble(1.0).toFloat())
                "setSleep", "setSkip" -> {
                    // TODO(phase 1): custom SessionCommands handled by PlaybackService (sleep fade 10 s).
                }
                "unload" -> c.stop()
            }
            invoke.resolve()
        }
    }

    @Command
    fun snapshot(invoke: Invoke) {
        withController { c -> invoke.resolve(PlayerEvents.snapshot(c)) }
    }

    @Command
    fun secretGet(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        invoke.resolve(JSObject().put("value", secrets.get(args.key)))
    }

    @Command
    fun secretSet(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        secrets.set(args.key, args.value ?: "")
        invoke.resolve()
    }

    @Command
    fun secretDelete(invoke: Invoke) {
        secrets.delete(invoke.parseArgs(SecretArgs::class.java).key)
        invoke.resolve()
    }

    @Command
    fun installApk(invoke: Invoke) {
        val args = invoke.parseArgs(InstallArgs::class.java)
        ApkInstaller(activity).install(args.path) { result ->
            emit(JSObject().put("source", "update").put("event", result))
        }
        invoke.resolve()
    }

    @Command
    fun takeShared(invoke: Invoke) {
        invoke.resolve(JSObject().put("items", shares.drain()))
    }

    override fun onDestroy() {
        controllerFuture?.let { MediaController.releaseFuture(it) }
        controllerFuture = null
    }
}

/** Turns Media3 player callbacks into `PlayerEvent` JSON (tag "type", see streams-native-bridge proto). */
class PlayerEvents(private val emit: (JSObject) -> Unit) : Player.Listener {
    override fun onEvents(player: Player, events: Player.Events) {
        // Envelope = BridgeEvent JSON (tag "source"), so Rust forwards it without re-shaping.
        val event = JSObject().put("type", "snapshot").put("snapshot", snapshot(player))
        emit(JSObject().put("source", "player").put("event", event))
        // TODO(phase 1): segmentChanged on onMediaItemTransition, ended on STATE_ENDED, error on onPlayerError,
        // and a 250 ms ticker while playing (Media3 does not call back on position changes).
    }

    companion object {
        fun snapshot(p: Player): JSObject {
            val state = when {
                p.playerError != null -> "error"
                p.playbackState == Player.STATE_BUFFERING -> "buffering"
                p.playbackState == Player.STATE_ENDED -> "ended"
                p.playbackState == Player.STATE_IDLE -> "idle"
                p.isPlaying -> "playing"
                else -> "paused"
            }
            return JSObject()
                .put("state", JSObject().put("kind", state))
                .put("segment", p.currentMediaItemIndex) // TODO: DocumentPlayer maps item index → segment
                .put("segmentPosMs", p.currentPosition)
                .put("bufferedAheadMs", (p.totalBufferedDuration))
                .put("rate", p.playbackParameters.speed.toDouble())
                .put("sleepLeftMs", null as Any?)
                .put("atMs", android.os.SystemClock.elapsedRealtime())
        }
    }
}
