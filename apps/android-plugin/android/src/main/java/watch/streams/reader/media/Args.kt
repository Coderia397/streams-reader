// Argument classes for MediaPlugin commands. Field names match the camelCase JSON the Rust side sends
// (apps/android-plugin/src/lib.rs and streams-native-bridge proto); Tauri parses them with Jackson.
package watch.streams.reader.media

import app.tauri.annotation.InvokeArg
import app.tauri.plugin.Channel
import com.fasterxml.jackson.databind.JsonNode

@InvokeArg
class EventHandlerArgs {
    lateinit var handler: Channel
}

@InvokeArg
class MediaInfoArg {
    var title: String = ""
    var artist: String = ""
    var album: String = ""
    var artworkUrl: String? = null
}

@InvokeArg
class OpenArgs {
    lateinit var docId: String
    var segmentCount: Int = 0
    var estimatedMs: IntArray = IntArray(0)
    var startSegment: Int = 0
    var startMs: Int = 0
    lateinit var info: MediaInfoArg
    var artworkPath: String? = null
    var rate: Double = 1.0
    /** `Sleep` JSON: {"kind":"off"} | {"kind":"after","ms":N} | {"kind":"endOfSection"} */
    var sleep: JsonNode? = null
    /** `Skip` JSON: {"backMs":N,"forwardMs":N} */
    var skip: JsonNode? = null
    var autoplay: Boolean = false
}

@InvokeArg
class QueueItemArg {
    var segment: Int = 0
    lateinit var uri: String
    var durationMs: Int = 0
    var endsSection: Boolean = false
}

@InvokeArg
class AppendArgs {
    var items: Array<QueueItemArg> = emptyArray()
}

/** `PlayerCommand` JSON (tag "cmd"), kept as a tree and dispatched in [MediaPlugin.control]. */
@InvokeArg
class ControlArgs {
    lateinit var command: JsonNode
}

@InvokeArg
class SecretArgs {
    lateinit var key: String
    var value: String? = null
}

@InvokeArg
class InstallArgs {
    lateinit var path: String
}
