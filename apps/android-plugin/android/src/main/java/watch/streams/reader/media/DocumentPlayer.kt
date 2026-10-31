// A document as one item. ExoPlayer holds one MediaItem per segment (local WebM/Opus file), but the lock screen,
// notification and Bluetooth metadata should show the whole document: one title, one progress bar over all
// segments, seek anywhere. DocumentPlayer wraps ExoPlayer in a ForwardingSimpleBasePlayer that presents a single
// playlist item whose duration is the sum of segment durations (estimates until real ones arrive) and maps
// positions and seeks between document time and (segment item, offset).
package watch.streams.reader.media

import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.util.UnstableApi
import androidx.media3.common.ForwardingSimpleBasePlayer
import androidx.media3.exoplayer.ExoPlayer
import android.net.Uri

@UnstableApi
class DocumentPlayer(private val exo: ExoPlayer) : ForwardingSimpleBasePlayer(exo) {
    /** Duration of every segment in ms; estimated from text length until the segment is on disk. */
    private var durationsMs: IntArray = IntArray(0)
    /** Segment index of ExoPlayer item 0 (the playlist starts at the resume point, not at segment 0). */
    private var firstSegment: Int = 0
    private var metadata: MediaMetadata = MediaMetadata.EMPTY

    fun open(docId: String, estimatesMs: IntArray, startSegment: Int, title: String, artist: String,
             album: String, artwork: Uri?) {
        durationsMs = estimatesMs.copyOf()
        firstSegment = startSegment
        metadata = MediaMetadata.Builder()
            .setTitle(title).setArtist(artist).setAlbumTitle(album)
            .setArtworkUri(artwork)
            .setMediaType(MediaMetadata.MEDIA_TYPE_AUDIO_BOOK_CHAPTER)
            .build()
        exo.clearMediaItems()
        // TODO(phase 1): invalidateState() after storing docId so the session publishes the new metadata.
    }

    /** Append segments in order (the feeder only sends the next contiguous ones). */
    fun append(items: Array<QueueItemArg>) {
        for (item in items) {
            if (item.segment < durationsMs.size) durationsMs[item.segment] = item.durationMs
            exo.addMediaItem(
                MediaItem.Builder()
                    .setMediaId("seg:${item.segment}")
                    .setUri(item.uri)
                    .build(),
            )
        }
        if (exo.playbackState == androidx.media3.common.Player.STATE_IDLE) exo.prepare()
    }

    /** Document position (ms) of segment `s` offset `ms`. */
    fun documentMs(segment: Int, ms: Long): Long =
        durationsMs.take(segment.coerceIn(0, durationsMs.size)).sumOf { it.toLong() } + ms

    // TODO(phase 1): override getState() to expose one MediaItemData (metadata above, duration = sum of
    // durationsMs) with contentPositionMs = documentMs(firstSegment + exo.currentMediaItemIndex,
    // exo.currentPosition), and handleSeek() mapping a document position back to (item, offset); seeking past the
    // last appended item asks the feeder (event "starved") and buffers.
    //
    // Gapless note (verified 2026-10-30): Media3 1.11.1 trims the Opus pre-skip but not the WebM end padding
    // (fixed in 1.12.0-alpha01, androidx/media#2576). Our segments end in 250–1200 ms of appended silence, so the
    // extra few ms are silence, not a click; positions stay exact because they are per item.
}
