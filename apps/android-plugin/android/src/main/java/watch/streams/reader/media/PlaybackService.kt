// Foreground media service (Media3 1.11.1). Owns the ExoPlayer and the MediaSession; Media3 posts and updates the
// MediaStyle notification itself (DefaultMediaNotificationProvider; from API 33 System UI draws it from the session),
// and handles lock screen, Bluetooth / headset buttons and Android Auto browsing later (MediaLibraryService).
package watch.streams.reader.media

import android.app.PendingIntent
import android.content.Intent
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.Player
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null
    private lateinit var document: DocumentPlayer

    override fun onCreate() {
        super.onCreate()
        val exo = ExoPlayer.Builder(this)
            // Speech content; ExoPlayer requests and abandons audio focus itself (calls, navigation prompts duck/pause).
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(C.USAGE_MEDIA)
                    .setContentType(C.AUDIO_CONTENT_TYPE_SPEECH)
                    .build(),
                /* handleAudioFocus = */ true,
            )
            // Pause when headphones are unplugged / Bluetooth drops ("becoming noisy").
            .setHandleAudioBecomingNoisy(true)
            // Keep the CPU awake while playing local files with the screen off.
            .setWakeMode(C.WAKE_MODE_LOCAL)
            // Seek-back / forward buttons on the notification use these increments (updated by SET_SKIP).
            .setSeekBackIncrementMs(15_000)
            .setSeekForwardIncrementMs(15_000)
            .build()
        // Offload stays off: Opus has no gapless in offload mode (androidx/media#2576).
        document = DocumentPlayer(exo)

        val launch = packageManager.getLaunchIntentForPackage(packageName)?.let {
            PendingIntent.getActivity(this, 0, it, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        }
        session = MediaSession.Builder(this, document)
            .apply { launch?.let { setSessionActivity(it) } }
            // TODO(phase 1): setCallback(...) accepting custom SessionCommands OPEN / APPEND / SET_SLEEP / SET_SKIP
            // from our own MediaController (package check), and setCustomLayout with ±15 s CommandButtons.
            .build()
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** Swiped away from recents: keep playing if playing, else stop the service (Media3 guidance). */
    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0 ||
            player.playbackState == Player.STATE_ENDED
        ) {
            stopSelf()
        }
    }

    override fun onDestroy() {
        session?.run {
            player.release()
            release()
        }
        session = null
        super.onDestroy()
    }
}
