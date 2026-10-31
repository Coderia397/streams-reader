# Streams Reader, Deutsch

app-name = Streams Reader

## Common actions
action-back = Zurück
action-cancel = Abbrechen
action-close = Schließen
action-delete = Löschen
action-dismiss = Ausblenden
action-not-now = Nicht jetzt
action-retry = Erneut versuchen
toast-reload = Neu laden
toast-undo = Rückgängig
toast-update-ready = Eine neue Version ist bereit.

## Navigation
nav-label = Hauptmenü
nav-library = Bibliothek
nav-voices = Stimmen
nav-settings = Einstellungen

## Times and numbers
time-left-under-minute = Noch weniger als eine Minute
time-left-min = Noch { $minutes } Min.
time-left-hours = Noch { $hours } Std. { $minutes } Min.
duration-min = { $minutes } Min.
duration-hours = { $hours } Std. { $minutes } Min.
# Storage sizes (decimal units) and the playback speed; NUMBER() writes the language's decimal separator.
size-b = { NUMBER($n, maximumFractionDigits: 0) } B
size-kb = { NUMBER($n, maximumFractionDigits: 1) } kB
size-mb = { NUMBER($n, maximumFractionDigits: 1) } MB
size-gb = { NUMBER($n, maximumFractionDigits: 1) } GB
speed-value = { NUMBER($rate, maximumFractionDigits: 2) }×
seconds-short = { $seconds } s
wait-under-minute = weniger als einer Minute
wait-minutes = { $minutes ->
    [one] einer Minute
   *[other] { $minutes } Minuten
}
wait-hours = { $hours ->
    [one] etwa einer Stunde
   *[other] etwa { $hours } Stunden
}

## Errors
error-offline = Du bist offline. Prüfe die Verbindung und versuch es noch einmal.
error-unexpected = Der Server hat unerwartet geantwortet. Versuch es gleich noch einmal.
error-browser = Dein Browser hat abgelehnt: { $detail }
error-bad-request = Diese Anfrage war nicht ganz richtig. Versuch es noch einmal.
error-unauthorized = Bitte melde dich erneut an.
error-forbidden = Das ist nicht erlaubt.
error-not-found = Das gibt es nicht mehr.
error-conflict = Das wurde woanders geändert. Aktualisiere und versuch es erneut.
error-too-large = Das ist zu groß zum Hochladen.
error-rate-limited = Zu viele Versuche. Warte kurz und versuch es dann erneut.
error-rate-limited-wait = Zu viele Versuche. Versuch es in { $wait } erneut.
error-gpu-budget = Das heutige Kontingent für ausdrucksstarke Stimmen ist aufgebraucht. Probier eine Standardstimme.
error-gpu-budget-wait = Das heutige Kontingent für ausdrucksstarke Stimmen ist aufgebraucht. Es erneuert sich in { $wait }.
error-engine-waking = Die Sprachausgabe startet gerade. Das kann eine Minute dauern.
error-engine-failed = Die Sprachausgabe konnte diese Stelle nicht lesen.
error-upstream = Die Seite ließ sich nicht laden. Prüfe den Link oder füge den Text direkt ein.
error-upgrade = Eine neue Version ist da. Wird neu geladen…
error-internal = Auf dem Server ist etwas schiefgegangen. Versuch es noch einmal.

## Sign in
sign-in-lead = Deine Leseliste, vorgelesen.
sign-in-title = Bei Streams Reader anmelden
sign-in-email-label = E-Mail
sign-in-email-placeholder = du@beispiel.de
sign-in-email-invalid = Das sieht nicht nach einer E-Mail-Adresse aus.
sign-in-continue = Mit E-Mail fortfahren
sign-in-or = oder
sign-in-with = Weiter mit { $provider }
sign-in-invite-label = Einladungscode
sign-in-invite-placeholder = Zum Beispiel STREAMS-1234
sign-in-invite-hint = Streams Reader ist vorerst nur auf Einladung nutzbar. Frag die Person, die dich eingeladen hat, nach dem Code.
sign-in-code-title = Schau in dein Postfach
sign-in-code-label = 6-stelliger Code
sign-in-verify = Anmelden
sign-in-link-hint = Du kannst auch den Link in dieser E-Mail auf diesem Gerät öffnen.
sign-in-resend = Neuen Code senden
sign-in-resend-wait = Erneut senden in { $seconds } s
sign-in-other-email = Andere E-Mail verwenden
sign-in-code-sent-before = Wir haben einen 6-stelligen Code an{" "}
sign-in-code-sent-after = {" "}gesendet.
sign-in-new-here = Neu hier? Gib deinen Einladungscode ein, um dein Konto zu erstellen.
sign-in-have-invite = Ich habe einen Einladungscode
sign-in-oauth-invite = Um so ein Konto zu erstellen, gib zuerst deine E-Mail-Adresse und deinen Einladungscode ein.
sign-in-legal-before = Wenn du fortfährst, stimmst du den{" "}
sign-in-legal-and = {" "}und der{" "}
sign-in-legal-after = {" "}zu.
legal-terms = Nutzungsbedingungen
legal-privacy = Datenschutzerklärung
auth-error-offline = Der Anmeldedienst ist nicht erreichbar. Prüfe deine Verbindung.
auth-error-code = Der Code ist falsch oder abgelaufen. Versuch es erneut oder fordere einen neuen an.
auth-invite-needed = Für ein Konto brauchst du einen Einladungscode.
auth-error-invite = Dieser Einladungscode ist ungültig oder wurde schon benutzt.
auth-error-rate = Zu viele Versuche. Warte eine Minute und versuch es dann erneut.
auth-error-session = Du wurdest abgemeldet. Bitte melde dich erneut an.
auth-error-other = Die Anmeldung hat nicht geklappt. Versuch es gleich noch einmal.
invite-used = Dieser Einladungscode wurde bereits verwendet.
invite-expired = Dieser Einladungscode ist abgelaufen. Bitte um einen neuen.
invite-taken = Dieser Einladungscode gehört zu einer anderen E-Mail-Adresse.
invite-unknown = Diesen Einladungscode kennen wir nicht. Prüfe ihn und versuche es erneut.
auth-callback-busy = Du wirst angemeldet…
auth-callback-failed = Die Anmeldung wurde nicht abgeschlossen
auth-callback-back = Zurück zur Anmeldung

## First run
welcome-title = Willkommen bei Streams Reader
welcome-lead = Zwei kurze Schritte, damit alles flüssig läuft – auch bei ausgeschaltetem Bildschirm.
welcome-install-title = Zum Startbildschirm hinzufügen
welcome-install-line = Die App öffnet sich im Vollbild und erscheint in den Mediensteuerungen.
welcome-install = Installieren
welcome-installed = Installiert
welcome-installed-toast = Installiert. Öffne die App über den Startbildschirm.
welcome-install-manual = Öffne in Chrome das Menü ⋮ und wähle Zum Startbildschirm hinzufügen.
welcome-background-title = Im Hintergrund weiterspielen lassen
welcome-background-line = Android kann den Ton einige Minuten nach dem Ausschalten des Bildschirms anhalten. Erlaube Chrome die Hintergrundnutzung:
welcome-android-1 = Öffne Einstellungen, dann Apps, dann Chrome.
welcome-android-2 = Tippe auf Akkunutzung der App.
welcome-android-3 = Wähle Uneingeschränkt.
welcome-samsung-line = Auf Samsung-Geräten nimm Chrome außerdem aus den Ruhelisten:
welcome-samsung-1 = Öffne Einstellungen, dann Akku, dann Einschränkungen der Hintergrundnutzung.
welcome-samsung-2 = Entferne Chrome aus Apps im Standby-Modus und Apps im Tiefschlaf.
welcome-keys-title = Mit der Tastatur hören
welcome-keys-line = Im Lesemodus startet und pausiert die Leertaste, J und L springen, [ und ] ändern das Tempo.
welcome-start = Jetzt zuhören

## Library
library-title = Bibliothek
library-search = In deiner Bibliothek suchen
library-search-placeholder = Titel, Autor oder Website
library-search-empty-line = Probier ein anderes Wort oder such im Lesemodus innerhalb eines Dokuments.
library-continue = Weiterhören
library-all = Alle Dokumente
library-more = Weitere Aktionen
library-play = Abspielen
library-delete = Aus der Bibliothek löschen
library-removed = „{ $title }“ wurde entfernt.
library-empty-title = Noch nichts zum Anhören
library-empty-line = Füge einen Artikel, ein Buch oder eigene Notizen hinzu und drück auf Abspielen.
library-share-tip = Tipp: Teile eine beliebige Seite aus deinem Browser mit Streams Reader.
library-unreachable = Deine Bibliothek ist nicht erreichbar
library-unreachable-line = Prüfe deine Verbindung und versuch es dann noch einmal.
library-preparing = Wird vorbereitet…
library-upload-failed = Hochladen nicht abgeschlossen
library-retry-gone = Diese Datei ist nicht mehr auf diesem Gerät. Lösch sie und füge sie erneut hinzu.
doc-finished = Fertig gehört
source-web = Webseite
source-text = Text
source-book = Buch
source-pdf = PDF
source-text-file = Textdatei
source-markdown = Markdown

## Adding documents
add-title = Zur Bibliothek hinzufügen
add-paste = Text einfügen
add-paste-hint = Ein Artikel, ein Kapitel, was du willst
add-link = Link einfügen
add-link-hint = Am besten für Artikel und Blogbeiträge
add-file = Datei öffnen
add-file-hint = EPUB, PDF, TXT, Markdown oder HTML
add-paste-title-label = Titel (optional)
add-paste-text-label = Text
add-link-label = Webadresse
add-paste-placeholder = Füge den Text hier ein oder tippe ihn.
add-paste-submit = Text hinzufügen
add-paste-empty = Füge zuerst etwas Text ein.
add-link-help = Die Seite wird bereinigt, damit nur der eigentliche Inhalt vorgelesen wird.
add-link-submit = Link hinzufügen
add-link-invalid = Das sieht nicht nach einer Webadresse aus.
add-file-error = Die Datei ließ sich nicht öffnen: { $detail }
add-share-tip = Du kannst Seiten und Dateien auch aus anderen Apps an Streams Reader teilen.
add-done = Zur Bibliothek hinzugefügt.
import-stage-fetching = Seite wird geladen…
import-stage-reading = Wird gelesen…
import-stage-uploading = Wird zur Bibliothek hinzugefügt…
import-error-empty = Darin ist kein lesbarer Text.
import-error-too-large = Das ist zu groß ({ $detail }).
import-error-damaged = Die Datei scheint beschädigt zu sein und ließ sich nicht lesen.
import-error-unsupported = Nicht unterstützt: { $detail }.
import-error-pdf = Das PDF ließ sich nicht lesen ({ $detail }).
import-error-file-type = Dieser Dateityp wird nicht unterstützt. Probier EPUB, PDF, TXT, Markdown oder HTML.
import-error-worker = Der Import ist nicht gestartet. Lade die Seite neu und versuch es noch einmal.

## Reader and player
reader-back = Zurück zur Bibliothek
reader-more = Mehr
reader-play-here = Ab hier abspielen
reader-back-to-voice = Zurück zur Stimme
reader-end = Ende
reader-no-text = Dieses Dokument enthält keinen lesbaren Text.
reader-hide-text = Ohne Text hören
reader-hide-text-hint = Beim Abspielen ist nur das Cover zu sehen
reader-show-text = Text anzeigen
reader-text-size = Textgröße
bookmark-add = Diese Stelle merken
bookmark-added = Lesezeichen gespeichert.
bookmark-exists = Für diese Stelle gibt es schon ein Lesezeichen.
bookmark-remove = Lesezeichen entfernen
reader-text-smaller = Kleinerer Text
reader-text-larger = Größerer Text
player-play = Abspielen
player-pause = Pausieren
player-now-playing = Läuft gerade
player-position = Position im Dokument
player-chapter-left = Noch { $time } im Kapitel
player-skip-back = { $seconds } Sekunden zurück
player-skip-forward = { $seconds } Sekunden vor
player-error = Die Wiedergabe wurde unerwartet angehalten. Drück auf Abspielen, um es erneut zu versuchen.
contents-title = Inhalt
search-doc-title = Im Text suchen
search-doc-placeholder = Wort oder Ausdruck suchen
search-doc-hint = Gib mindestens zwei Buchstaben ein.
search-doc-count = { $count ->
    [one] Ein Treffer
   *[other] { $count } Treffer
}
search-no-results = Keine Treffer
keys-title = Tastenkürzel
keys-toggle = Abspielen oder pausieren
keys-skip = Zurück- oder vorspringen
keys-speed = Langsamer oder schneller
keys-paragraph = Vorheriger oder nächster Absatz
keys-follow = Zurück zur Stimme
keys-search = Im Text suchen

## Speed and sleep
speed-title = Wiedergabetempo
speed-slower = Langsamer
speed-faster = Schneller
speed-hint = Die Stimmlage bleibt bei jedem Tempo natürlich.
sleep-title = Schlaftimer
sleep-on = Schlaftimer an
sleep-off = Aus
sleep-minutes = { $minutes } Min.
sleep-end-chapter = Ende des Kapitels
sleep-stops-in = Stoppt in { $time }
sleep-hint = Die letzten zehn Sekunden werden sanft ausgeblendet.
sleep-set = Schlaftimer: { $choice }
sleep-cleared = Schlaftimer aus
sleep-ended = Der Schlaftimer ist abgelaufen. Schlaf gut.

## Voices
voices-title = Stimmen
voices-lead = Wähle die Stimme, die dir vorliest. Tippe auf den Lautsprecher, um sie vorher zu hören.
voices-favourites = Favoriten
voices-recent = Zuletzt verwendet
voices-yours = Deine Stimmen
voices-standard = Standard
voices-standard-hint = Unbegrenzt hören. Startet schnell.
voices-expressive = Ausdrucksstark
voices-expressive-hint = Nutzt das tägliche Kontingent.
voices-expressive-line = Reicher, gefühlvoller Vortrag. Jede Hörprobe und jede Audiominute zählt zum Tageskontingent.
voices-budget-today = Heutiges Kontingent
voices-budget-numbers = { $left } von { $cap } s · { $runs ->
        [one] { $runs } Durchlauf
       *[other] { $runs } Durchläufe
    }
voices-more = Mehr Stimmen
voices-fewer = Weniger Stimmen
voice-title = Stimme
voice-switched = Jetzt liest { $name }
voice-default-set = { $name } ist jetzt deine Standardstimme
voice-budget-left = Heute noch etwa { $minutes } Min. ausdrucksstarkes Audio und { $runs ->
        [one] { $runs } Durchlauf
       *[other] { $runs } Durchläufe
    }.
voice-badge-expressive = Ausdrucksstark
voice-favourite = Zu Favoriten hinzufügen
voice-unfavourite = Aus Favoriten entfernen
voice-preview = { $name } anhören
voice-manage = Stimme verwalten
voice-manage-line = Beim Löschen werden die Stimme und die Aufnahme entfernt, aus der sie entstanden ist.
voice-delete = Diese Stimme löschen
voice-deleted = { $name } wurde gelöscht.
voice-ask-title = Ausdrucksstarke Hörprobe abspielen?
voice-ask-line = Dafür wird eine kurze Probe auf der GPU erzeugt. Das kostet etwa { $seconds } Sekunden des heutigen Kontingents.
voice-ask-left = Heute noch { $left ->
        [one] { $left } Sekunde
       *[other] { $left } Sekunden
    } und { $runs ->
        [one] { $runs } Durchlauf
       *[other] { $runs } Durchläufe
    }.
voice-ask-play = Hörprobe abspielen
voice-state-needs-recording = Aufnahme fehlt
voice-state-preparing = wird vorbereitet…
voice-state-failed = fehlgeschlagen
voice-yours-fallback = Deine Stimme

accent-american = Amerikanisch
accent-british = Britisch
gender-female = Weiblich
gender-male = Männlich

## Recording a voice
record-title = Deine Stimme aufnehmen
record-cta = Deine Stimme aufnehmen
record-cta-line = Lies acht kurze Sätze vor. Etwa zwei Minuten.
record-tips = Such dir einen ruhigen Raum, halte das Handy eine Handbreit vom Mund entfernt und lies jeden englischen Satz mit dem angegebenen Gefühl.
record-name = Name
record-default-name = Meine Stimme
record-start = Aufnehmen
record-stop = Stopp
record-again = Neu aufnehmen
record-progress = { $done } von { $total } Sätzen aufgenommen
record-create = Stimme erstellen
record-uploading = Aufnahme wird hochgeladen…
record-preparing = Deine Stimme wird vorbereitet…
record-wait = Das dauert ein bis zwei Minuten. Du kannst diese Ansicht verlassen; es läuft weiter.
record-ready = Deine Stimme ist bereit
record-ready-line = Sie liest mit der ausdrucksstarken Sprachausgabe, die das Tageskontingent nutzt.
record-use = Meine Stimme verwenden
record-failed-title = Das hat nicht geklappt
record-back = Zurück zur Aufnahme
record-line-good = Satz { $line } klingt gut.
record-failed = Aufnahme fehlgeschlagen: { $detail }
record-every-line = Nimm zuerst alle Sätze auf.
record-prepare-failed = Aus der Aufnahme ließ sich keine Stimme erstellen. Versuch es an einem ruhigeren Ort.
record-vanished = Die Stimme ist während der Vorbereitung verschwunden.
record-still-preparing = Wird noch vorbereitet. Sie erscheint unter Deine Stimmen, sobald sie fertig ist.
record-error-blocked = Der Mikrofonzugriff ist blockiert. Erlaube ihn in den Website-Einstellungen des Browsers und versuch es erneut.
record-error-no-mic = Kein Mikrofon gefunden.
record-error-other = Aufnahme konnte nicht starten: { $detail }
record-issue-clipping = Zu laut, es hat übersteuert. Geh etwas zurück und lies den Satz noch einmal.
record-issue-noisy = Im Hintergrund ist es laut. Such dir wenn möglich einen ruhigeren Ort.
record-issue-quiet = Sehr leise. Halte das Handy etwas näher.
record-issue-short = Das war sehr kurz. Lies den ganzen Satz.
mood-warm = Warm
mood-calm = Ruhig
mood-delighted = Begeistert
mood-sorry = Bedauernd
mood-firm = Bestimmt
mood-curious = Neugierig
mood-amused = Amüsiert
mood-gentle = Sanft

## Settings
settings-title = Einstellungen
settings-account = Konto
settings-listening = Hören
settings-appearance = Darstellung
settings-storage = Speicher
settings-engine = Sprachausgabe
settings-about = Über
settings-default-voice = Standardstimme
settings-default-speed = Standardtempo
settings-skip-back = Zurückspringen
settings-skip-forward = Vorspringen
settings-theme = Design
settings-language = Sprache
theme-ink = Tinte
theme-paper = Papier
theme-night = Nacht
theme-system = System
language-auto = Wie dieses Gerät
language-auto-named = Automatisch ({ $language })
account-email = E-Mail
account-offline = Offline
account-method-email = E-Mail-Code
account-unlink = Trennen
account-unlinked = Anmeldemethode entfernt.
account-methods = Anmeldemethoden
account-linked = Verknüpft
account-not-linked = Nicht verknüpft
account-link-short = Verknüpfen
account-sign-out = Abmelden
account-sign-out-all = Auf allen Geräten abmelden
account-sign-out-all-confirm = Auf allen Geräten abmelden?
account-sign-out-all-line = Jedes Telefon, Tablet und jeder Computer mit diesem Konto muss sich neu anmelden.
account-sign-out-all-action = Überall abmelden
storage-used = Auf diesem Gerät belegt
storage-clear = Gespeichertes Audio löschen
storage-hint = Bereits gehörtes Audio bleibt gespeichert, damit es offline läuft.
storage-clear-title = Gespeichertes Audio löschen?
storage-clear-line = Das schafft Platz. Die Abschnitte werden bei Bedarf neu geladen.
storage-clear-confirm = Löschen
storage-cleared = Gespeichertes Audio gelöscht.
engine-voice = Status
engine-awake = Bereit
engine-asleep = Im Ruhezustand (startet bei der ersten Nutzung)
engine-starting = Startet
engine-unavailable = Nicht verfügbar
engine-gpu-today = Kontingent heute
engine-gpu-left = { $left } von { $cap } s · { $runs ->
        [one] { $runs } Durchlauf
       *[other] { $runs } Durchläufe
    }
engine-queue = Warten auf Erzeugung
engine-refresh = Status aktualisieren
about-version = Version
about-setup = Installation und Wiedergabe im Hintergrund
about-source = Quellcode
about-colophon = Streams Reader · kostenlos und quelloffen

## Not found
not-found-title = Hier ist nichts
not-found-line = Diese Seite gibt es nicht.
not-found-back = Zurück zur Bibliothek
