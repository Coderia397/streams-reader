// A cross-origin frame inside the app pretends to be the app and asks for the handoff. app-bridge.js must ignore it.
parent.postMessage({ source: "streams-reader-app", type: "handoff:ready", v: 1, requestId: "spoof" }, "*");
