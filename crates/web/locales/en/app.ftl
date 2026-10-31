# Streams Reader, English (the reference: every other language has exactly these keys and placeholders).

app-name = Streams Reader

## Common actions
action-back = Back
action-cancel = Cancel
action-close = Close
action-delete = Delete
action-dismiss = Dismiss
action-not-now = Not now
action-retry = Try again
toast-reload = Reload
toast-undo = Undo
toast-update-ready = A new version is ready.

## Navigation
nav-label = Main
nav-library = Library
nav-voices = Voices
nav-settings = Settings

## Times and numbers
time-left-under-minute = Under a minute left
time-left-min = { $minutes } min left
time-left-hours = { $hours } h { $minutes } min left
duration-min = { $minutes } min
duration-hours = { $hours } h { $minutes } min
# Storage sizes (decimal units) and the playback speed; NUMBER() writes the language's decimal separator.
size-b = { NUMBER($n, maximumFractionDigits: 0) } B
size-kb = { NUMBER($n, maximumFractionDigits: 1) } kB
size-mb = { NUMBER($n, maximumFractionDigits: 1) } MB
size-gb = { NUMBER($n, maximumFractionDigits: 1) } GB
speed-value = { NUMBER($rate, maximumFractionDigits: 2) }×
seconds-short = { $seconds } s
wait-under-minute = under a minute
wait-minutes = { $minutes ->
    [one] a minute
   *[other] { $minutes } minutes
}
wait-hours = { $hours ->
    [one] about an hour
   *[other] about { $hours } hours
}

## Errors
error-offline = You're offline. Check your connection and try again.
error-unexpected = The server sent something unexpected. Try again in a moment.
error-browser = Your browser refused: { $detail }
error-bad-request = That request wasn't quite right. Try again.
error-unauthorized = Please sign in again.
error-forbidden = That isn't allowed.
error-not-found = That item no longer exists.
error-conflict = That changed somewhere else. Refresh and try again.
error-too-large = That's too large to upload.
error-rate-limited = Too many attempts. Wait a little and try again.
error-rate-limited-wait = Too many attempts. Try again in { $wait }.
error-gpu-budget = Today's expressive voice time is used up. Try a Standard voice.
error-gpu-budget-wait = Today's expressive voice time is used up. It frees up in { $wait }.
error-engine-waking = The voice engine is waking up. This can take a minute.
error-engine-failed = The voice engine couldn't read that part.
error-upstream = Couldn't load that page. Check the link or paste the text instead.
error-upgrade = A new version is available. Reloading…
error-internal = Something went wrong on the server. Try again.

## Sign in
sign-in-lead = Your reading list, read aloud.
sign-in-title = Sign in to Streams Reader
sign-in-email-label = Email
sign-in-email-placeholder = you@example.com
sign-in-email-invalid = That doesn't look like an email address.
sign-in-continue = Continue with email
sign-in-or = or
sign-in-with = Continue with { $provider }
sign-in-invite-label = Invite code
sign-in-invite-placeholder = For example STREAMS-1234
sign-in-invite-hint = Streams Reader is invite-only for now. Ask whoever invited you for the code.
sign-in-code-title = Check your email
sign-in-code-label = 6-digit code
sign-in-verify = Sign in
sign-in-link-hint = You can also tap the link in that email on this device.
sign-in-resend = Send a new code
sign-in-resend-wait = Send again in { $seconds } s
sign-in-other-email = Use another email
sign-in-code-sent-before = We sent a 6-digit code to{" "}
sign-in-code-sent-after = .
sign-in-new-here = New here? Add your invite code to create your account.
sign-in-have-invite = I have an invite code
sign-in-oauth-invite = To create an account this way, enter your email and invite code first.
sign-in-legal-before = By continuing you agree to the{" "}
sign-in-legal-and = {" "}and the{" "}
sign-in-legal-after = .
legal-terms = Terms of Use
legal-privacy = Privacy Policy
auth-error-offline = Can't reach the sign-in service. Check your connection.
auth-error-code = That code is wrong or has expired. Try again or send a new one.
auth-invite-needed = You need an invite code to create an account.
auth-error-invite = That invite code isn't valid or has been used.
auth-error-rate = Too many tries. Wait a minute, then try again.
auth-error-session = You've been signed out. Please sign in again.
auth-error-other = Sign-in didn't work. Try again in a moment.
invite-used = That invite code has already been used.
invite-expired = That invite code has expired. Ask for a new one.
invite-taken = That invite code belongs to another email address.
invite-unknown = We don't recognise that invite code. Check it and try again.
auth-callback-busy = Signing you in…
auth-callback-failed = That sign-in didn't finish
auth-callback-back = Back to sign in

## First run
welcome-title = Welcome to Streams Reader
welcome-lead = Two quick things so it plays smoothly, even with the screen off.
welcome-install-title = Add it to your home screen
welcome-install-line = It opens full screen and shows up in your media controls.
welcome-install = Install
welcome-installed = Installed
welcome-installed-toast = Installed. Open it from your home screen.
welcome-install-manual = In Chrome, open the ⋮ menu and choose Add to Home screen.
welcome-background-title = Let it play in the background
welcome-background-line = Android can pause audio a few minutes after the screen turns off. Allow Chrome to run in the background:
welcome-android-1 = Open Settings, then Apps, then Chrome.
welcome-android-2 = Tap App battery usage.
welcome-android-3 = Choose Unrestricted.
welcome-samsung-line = On Samsung phones, also take Chrome off the sleeping lists:
welcome-samsung-1 = Open Settings, then Battery, then Background usage limits.
welcome-samsung-2 = Remove Chrome from Sleeping apps and Deep sleeping apps.
welcome-keys-title = Listen with the keyboard
welcome-keys-line = In the reader, Space plays and pauses, J and L skip, and [ and ] change the speed.
welcome-start = Start listening

## Library
library-title = Library
library-search = Search your library
library-search-placeholder = Title, author or site
library-search-empty-line = Try another word, or search inside a document from the reader.
library-continue = Continue listening
library-all = All documents
library-more = More actions
library-play = Play
library-delete = Delete from library
library-removed = Removed “{ $title }”.
library-empty-title = Nothing to listen to yet
library-empty-line = Add an article, a book or your own notes, then press play.
library-share-tip = Tip: share any page from your browser to Streams Reader.
library-unreachable = Can't reach your library
library-unreachable-line = Check your connection, then try again.
library-preparing = Preparing…
library-upload-failed = Upload didn't finish
library-retry-gone = This file is no longer on this device. Delete it, then add it again.
doc-finished = Finished
source-web = Web page
source-text = Text
source-book = Book
source-pdf = PDF
source-text-file = Text file
source-markdown = Markdown

## Adding documents
add-title = Add to library
add-paste = Paste text
add-paste-hint = An article, a chapter, anything
add-link = Paste a link
add-link-hint = Articles and blog posts work best
add-file = Open a file
add-file-hint = EPUB, PDF, TXT, Markdown or HTML
add-paste-title-label = Title (optional)
add-paste-text-label = Text
add-link-label = Web address
add-paste-placeholder = Paste or type the text here.
add-paste-submit = Add text
add-paste-empty = Paste some text first.
add-link-help = The page is cleaned up so only the story is read.
add-link-submit = Add link
add-link-invalid = That doesn't look like a web address.
add-file-error = Couldn't open that file: { $detail }
add-share-tip = You can also share a page or a file to Streams Reader from other apps.
add-done = Added to your library.
import-stage-fetching = Fetching the page…
import-stage-reading = Reading…
import-stage-uploading = Adding to your library…
import-error-empty = There's no readable text in that.
import-error-too-large = That's too large ({ $detail }).
import-error-damaged = That file looks damaged and couldn't be read.
import-error-unsupported = Not supported: { $detail }.
import-error-pdf = Couldn't read that PDF ({ $detail }).
import-error-file-type = That file type isn't supported. Try EPUB, PDF, TXT, Markdown or HTML.
import-error-worker = The importer didn't start. Reload the page and try again.

## Reader and player
reader-back = Back to library
reader-more = More
reader-play-here = Play from here
reader-back-to-voice = Back to voice
reader-end = The end
reader-no-text = This document has no readable text.
reader-hide-text = Listen without text
reader-hide-text-hint = Shows only the cover while it plays
reader-show-text = Show the text
reader-text-size = Text size
bookmark-add = Bookmark this place
bookmark-added = Bookmark saved.
bookmark-exists = This place is already bookmarked.
bookmark-remove = Remove bookmark
reader-text-smaller = Smaller text
reader-text-larger = Larger text
player-play = Play
player-pause = Pause
player-now-playing = Now playing
player-position = Position in document
player-chapter-left = { $time } left in chapter
player-skip-back = Back { $seconds } seconds
player-skip-forward = Forward { $seconds } seconds
player-error = Playback stopped unexpectedly. Press play to try again.
contents-title = Contents
search-doc-title = Search in text
search-doc-placeholder = Find a word or phrase
search-doc-hint = Type at least two letters.
search-doc-count = { $count ->
    [one] One match
   *[other] { $count } matches
}
search-no-results = No matches
keys-title = Keyboard shortcuts
keys-toggle = Play or pause
keys-skip = Skip back or forward
keys-speed = Slower or faster
keys-paragraph = Previous or next paragraph
keys-follow = Back to voice
keys-search = Search in text

## Speed and sleep
speed-title = Playback speed
speed-slower = Slower
speed-faster = Faster
speed-hint = Pitch stays natural at every speed.
sleep-title = Sleep timer
sleep-on = Sleep timer on
sleep-off = Off
sleep-minutes = { $minutes } min
sleep-end-chapter = End of chapter
sleep-stops-in = Stops in { $time }
sleep-hint = The last ten seconds fade out gently.
sleep-set = Sleep timer: { $choice }
sleep-cleared = Sleep timer off
sleep-ended = Sleep timer ended. Sleep well.

## Voices
voices-title = Voices
voices-lead = Pick the voice that reads to you. Tap the speaker to hear it first.
voices-favourites = Favourites
voices-recent = Recent
voices-yours = Your voices
voices-standard = Standard
voices-standard-hint = Unlimited listening. Quick to start.
voices-expressive = Expressive
voices-expressive-hint = Uses the daily expressive budget.
voices-expressive-line = Richer, more emotional delivery. Each preview and minute of audio uses the daily budget.
voices-budget-today = Today's expressive budget
voices-budget-numbers = { $left } of { $cap } s · { $runs ->
        [one] { $runs } run
       *[other] { $runs } runs
    }
voices-more = More voices
voices-fewer = Fewer voices
voice-title = Voice
voice-switched = Now reading with { $name }
voice-default-set = { $name } is now your default voice
voice-budget-left = About { $minutes } min of expressive audio and { $runs ->
        [one] { $runs } run
       *[other] { $runs } runs
    } left today.
voice-badge-expressive = Expressive
voice-favourite = Add to favourites
voice-unfavourite = Remove from favourites
voice-preview = Preview { $name }
voice-manage = Manage voice
voice-manage-line = Deleting removes the voice and the recording it was made from.
voice-delete = Delete this voice
voice-deleted = Deleted { $name }.
voice-ask-title = Play an expressive preview?
voice-ask-line = This renders a short sample on the GPU and uses about { $seconds } seconds of today's budget.
voice-ask-left = { $left ->
        [one] { $left } second
       *[other] { $left } seconds
    } and { $runs ->
        [one] { $runs } run
       *[other] { $runs } runs
    } left today.
voice-ask-play = Play preview
voice-state-needs-recording = needs a recording
voice-state-preparing = preparing…
voice-state-failed = failed
voice-yours-fallback = Your voice

accent-american = American
accent-british = British
gender-female = Female
gender-male = Male

## Recording a voice
record-title = Record your voice
record-cta = Record your voice
record-cta-line = Read eight short lines. About two minutes.
record-tips = Find a quiet room, hold the phone a hand's width from your mouth, and read each English line the way it feels.
record-name = Name
record-default-name = My voice
record-start = Record
record-stop = Stop
record-again = Record again
record-progress = { $done } of { $total } lines recorded
record-create = Create voice
record-uploading = Uploading your recording…
record-preparing = Preparing your voice…
record-wait = This takes a minute or two. You can leave this screen; it carries on without you.
record-ready = Your voice is ready
record-ready-line = It reads with the expressive engine, which uses the daily budget.
record-use = Use my voice
record-failed-title = That didn't work
record-back = Back to recording
record-line-good = Line { $line } sounds good.
record-failed = Recording failed: { $detail }
record-every-line = Record every line first.
record-prepare-failed = The recording couldn't be turned into a voice. Try again somewhere quieter.
record-vanished = The voice disappeared while it was being prepared.
record-still-preparing = Still preparing. It will appear under Your voices when it's ready.
record-error-blocked = Microphone access is blocked. Allow it in the browser's site settings and try again.
record-error-no-mic = No microphone found.
record-error-other = Couldn't start recording: { $detail }
record-issue-clipping = Too loud: it distorted. Move back a little and read it again.
record-issue-noisy = There's background noise. Find a quieter spot if you can.
record-issue-quiet = Very quiet. Hold the phone a little closer.
record-issue-short = That was very short. Read the whole line.
mood-warm = Warm
mood-calm = Calm
mood-delighted = Delighted
mood-sorry = Sorry
mood-firm = Firm
mood-curious = Curious
mood-amused = Amused
mood-gentle = Gentle

## Settings
settings-title = Settings
settings-account = Account
settings-listening = Listening
settings-appearance = Appearance
settings-storage = Storage
settings-engine = Voice engine
settings-about = About
settings-default-voice = Default voice
settings-default-speed = Default speed
settings-skip-back = Skip back
settings-skip-forward = Skip forward
settings-theme = Theme
settings-language = Language
theme-ink = Ink
theme-paper = Paper
theme-night = Night
theme-system = System
language-auto = Same as this device
language-auto-named = Automatic ({ $language })
account-email = Email
account-offline = Offline
account-method-email = Email code
account-unlink = Unlink
account-unlinked = Sign-in method removed.
account-methods = Sign-in methods
account-linked = Linked
account-not-linked = Not linked
account-link-short = Link
account-sign-out = Sign out
account-sign-out-all = Sign out on every device
account-sign-out-all-confirm = Sign out on every device?
account-sign-out-all-line = Every phone, tablet and computer signed in to this account will need to sign in again.
account-sign-out-all-action = Sign out everywhere
storage-used = Used on this device
storage-clear = Clear saved audio
storage-hint = Audio you've listened to is kept so it replays offline.
storage-clear-title = Clear saved audio?
storage-clear-line = Clearing frees space. Clips download again when you need them.
storage-clear-confirm = Clear
storage-cleared = Saved audio cleared.
engine-voice = Status
engine-awake = Awake
engine-asleep = Asleep (wakes on first use)
engine-starting = Starting up
engine-unavailable = Unavailable
engine-gpu-today = Expressive budget today
engine-gpu-left = { $left } of { $cap } s · { $runs ->
        [one] { $runs } run
       *[other] { $runs } runs
    }
engine-queue = Waiting to render
engine-refresh = Refresh status
about-version = Version
about-setup = Install and background playback
about-source = Source code
about-colophon = Streams Reader · free and open source

## Not found
not-found-title = Nothing here
not-found-line = That page doesn't exist.
not-found-back = Back to the library
