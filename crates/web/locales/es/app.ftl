# Streams Reader, español

app-name = Streams Reader

## Common actions
action-back = Atrás
action-cancel = Cancelar
action-close = Cerrar
action-delete = Eliminar
action-dismiss = Descartar
action-not-now = Ahora no
action-retry = Reintentar
toast-reload = Recargar
toast-undo = Deshacer
toast-update-ready = Hay una versión nueva lista.

## Navigation
nav-label = Principal
nav-library = Biblioteca
nav-voices = Voces
nav-settings = Ajustes

## Times and numbers
time-left-under-minute = Queda menos de un minuto
time-left-min = Quedan { $minutes } min
time-left-hours = Quedan { $hours } h { $minutes } min
duration-min = { $minutes } min
duration-hours = { $hours } h { $minutes } min
# Storage sizes (decimal units) and the playback speed; NUMBER() writes the language's decimal separator.
size-b = { NUMBER($n, maximumFractionDigits: 0) } B
size-kb = { NUMBER($n, maximumFractionDigits: 1) } kB
size-mb = { NUMBER($n, maximumFractionDigits: 1) } MB
size-gb = { NUMBER($n, maximumFractionDigits: 1) } GB
speed-value = { NUMBER($rate, maximumFractionDigits: 2) }×
seconds-short = { $seconds } s
wait-under-minute = menos de un minuto
wait-minutes = { $minutes ->
    [one] un minuto
   *[other] { $minutes } minutos
}
wait-hours = { $hours ->
    [one] una hora aproximadamente
   *[other] unas { $hours } horas
}

## Errors
error-offline = No tienes conexión. Revísala y vuelve a intentarlo.
error-unexpected = El servidor respondió algo inesperado. Inténtalo de nuevo en un momento.
error-browser = Tu navegador lo rechazó: { $detail }
error-bad-request = La solicitud no era del todo correcta. Inténtalo otra vez.
error-unauthorized = Vuelve a iniciar sesión.
error-forbidden = Eso no está permitido.
error-not-found = Ese elemento ya no existe.
error-conflict = Eso cambió en otro lugar. Actualiza e inténtalo de nuevo.
error-too-large = Es demasiado grande para subirlo.
error-rate-limited = Demasiados intentos. Espera un poco y vuelve a probar.
error-rate-limited-wait = Demasiados intentos. Vuelve a probar en { $wait }.
error-gpu-budget = Se agotó el tiempo de voces expresivas de hoy. Prueba una voz estándar.
error-gpu-budget-wait = Se agotó el tiempo de voces expresivas de hoy. Se renueva en { $wait }.
error-engine-waking = El motor de voz se está despertando. Puede tardar un minuto.
error-engine-failed = El motor de voz no pudo leer esa parte.
error-upstream = No se pudo cargar la página. Revisa el enlace o pega el texto.
error-upgrade = Hay una versión nueva. Recargando…
error-internal = Algo falló en el servidor. Inténtalo de nuevo.

## Sign in
sign-in-lead = Tu lista de lectura, leída en voz alta.
sign-in-title = Inicia sesión en Streams Reader
sign-in-email-label = Correo electrónico
sign-in-email-placeholder = tu@ejemplo.com
sign-in-email-invalid = Eso no parece un correo electrónico.
sign-in-continue = Continuar con correo
sign-in-or = o
sign-in-with = Continuar con { $provider }
sign-in-invite-label = Código de invitación
sign-in-invite-placeholder = Por ejemplo STREAMS-1234
sign-in-invite-hint = Por ahora Streams Reader funciona solo con invitación. Pide el código a quien te invitó.
sign-in-code-title = Revisa tu correo
sign-in-code-label = Código de 6 dígitos
sign-in-verify = Iniciar sesión
sign-in-link-hint = También puedes abrir el enlace de ese correo en este dispositivo.
sign-in-resend = Enviar un código nuevo
sign-in-resend-wait = Reenviar en { $seconds } s
sign-in-other-email = Usar otro correo
sign-in-code-sent-before = Enviamos un código de 6 dígitos a{" "}
sign-in-code-sent-after = .
sign-in-new-here = ¿Eres nuevo? Añade tu código de invitación para crear tu cuenta.
sign-in-have-invite = Tengo un código de invitación
sign-in-oauth-invite = Para crear una cuenta así, escribe primero tu correo y tu código de invitación.
sign-in-legal-before = Al continuar aceptas los{" "}
sign-in-legal-and = {" "}y la{" "}
sign-in-legal-after = .
legal-terms = Términos
legal-privacy = Privacidad
auth-error-offline = No se puede contactar con el servicio de acceso. Revisa tu conexión.
auth-error-code = El código no es correcto o ha caducado. Inténtalo de nuevo o pide otro.
auth-invite-needed = Necesitas un código de invitación para crear una cuenta.
auth-error-invite = Ese código de invitación no es válido o ya se usó.
auth-error-rate = Demasiados intentos. Espera un minuto y vuelve a probar.
auth-error-session = Se cerró tu sesión. Vuelve a iniciarla.
auth-error-other = No se pudo iniciar sesión. Inténtalo en un momento.
invite-used = Ese código de invitación ya se ha usado.
invite-expired = Ese código de invitación ha caducado. Pide uno nuevo.
invite-taken = Ese código de invitación pertenece a otro correo.
invite-unknown = No reconocemos ese código de invitación. Revísalo e inténtalo de nuevo.
auth-callback-busy = Iniciando sesión…
auth-callback-failed = El inicio de sesión no terminó
auth-callback-back = Volver al inicio de sesión

## First run
welcome-title = Te damos la bienvenida a Streams Reader
welcome-lead = Dos pasos rápidos para que suene sin cortes, incluso con la pantalla apagada.
welcome-install-title = Añádelo a la pantalla de inicio
welcome-install-line = Se abre a pantalla completa y aparece en los controles multimedia.
welcome-install = Instalar
welcome-installed = Instalado
welcome-installed-toast = Instalado. Ábrelo desde la pantalla de inicio.
welcome-install-manual = En Chrome, abre el menú ⋮ y elige Añadir a pantalla de inicio.
welcome-background-title = Deja que suene en segundo plano
welcome-background-line = Android puede pausar el audio unos minutos después de apagar la pantalla. Permite que Chrome funcione en segundo plano:
welcome-android-1 = Abre Ajustes, luego Aplicaciones y luego Chrome.
welcome-android-2 = Toca Uso de batería de la aplicación.
welcome-android-3 = Elige Sin restricciones.
welcome-samsung-line = En los teléfonos Samsung, quita también Chrome de las listas de suspensión:
welcome-samsung-1 = Abre Ajustes, luego Batería y luego Límites de uso en segundo plano.
welcome-samsung-2 = Quita Chrome de Aplicaciones inactivas y de Aplicaciones en suspensión profunda.
welcome-keys-title = Escucha con el teclado
welcome-keys-line = En el lector, la barra espaciadora reproduce y pausa, J y L saltan, y [ y ] cambian la velocidad.
welcome-start = Empezar a escuchar

## Library
library-title = Biblioteca
library-search = Buscar en tu biblioteca
library-search-placeholder = Título, autor o sitio
library-search-empty-line = Prueba con otra palabra o busca dentro de un documento desde el lector.
library-continue = Seguir escuchando
library-all = Todos los documentos
library-more = Más acciones
library-play = Reproducir
library-delete = Eliminar de la biblioteca
library-removed = Se eliminó «{ $title }».
library-empty-title = Aún no hay nada que escuchar
library-empty-line = Añade un artículo, un libro o tus propias notas y pulsa reproducir.
library-share-tip = Consejo: comparte cualquier página desde tu navegador con Streams Reader.
library-unreachable = No se puede acceder a tu biblioteca
library-unreachable-line = Revisa tu conexión y vuelve a intentarlo.
library-preparing = Preparando…
library-upload-failed = La subida no terminó
library-retry-gone = Este archivo ya no está en este dispositivo. Elimínalo y vuelve a añadirlo.
doc-finished = Terminado
source-web = Página web
source-text = Texto
source-book = Libro
source-pdf = PDF
source-text-file = Archivo de texto
source-markdown = Markdown

## Adding documents
add-title = Añadir a la biblioteca
add-paste = Pegar texto
add-paste-hint = Un artículo, un capítulo, lo que quieras
add-link = Pegar un enlace
add-link-hint = Funciona mejor con artículos y blogs
add-file = Abrir un archivo
add-file-hint = EPUB, PDF, TXT, Markdown o HTML
add-paste-title-label = Título (opcional)
add-paste-text-label = Texto
add-link-label = Dirección web
add-paste-placeholder = Pega o escribe el texto aquí.
add-paste-submit = Añadir texto
add-paste-empty = Primero pega algo de texto.
add-link-help = La página se limpia para leer solo el contenido.
add-link-submit = Añadir enlace
add-link-invalid = Eso no parece una dirección web.
add-file-error = No se pudo abrir el archivo: { $detail }
add-share-tip = También puedes compartir una página o un archivo con Streams Reader desde otras apps.
add-done = Añadido a tu biblioteca.
import-stage-fetching = Descargando la página…
import-stage-reading = Leyendo…
import-stage-uploading = Añadiendo a tu biblioteca…
import-error-empty = No hay texto legible.
import-error-too-large = Es demasiado grande ({ $detail }).
import-error-damaged = El archivo parece dañado y no se pudo leer.
import-error-unsupported = No compatible: { $detail }.
import-error-pdf = No se pudo leer ese PDF ({ $detail }).
import-error-file-type = Ese tipo de archivo no es compatible. Prueba con EPUB, PDF, TXT, Markdown o HTML.
import-error-worker = No se pudo iniciar el importador. Recarga la página e inténtalo de nuevo.

## Reader and player
reader-back = Volver a la biblioteca
reader-more = Más
reader-play-here = Reproducir desde aquí
reader-back-to-voice = Volver a la voz
reader-end = Fin
reader-no-text = Este documento no tiene texto legible.
reader-hide-text = Escuchar sin texto
reader-hide-text-hint = Solo se ve la portada mientras suena
reader-show-text = Mostrar el texto
reader-text-size = Tamaño del texto
bookmark-add = Marcar este punto
bookmark-added = Marcador guardado.
bookmark-exists = Este punto ya tiene un marcador.
bookmark-remove = Quitar marcador
reader-text-smaller = Texto más pequeño
reader-text-larger = Texto más grande
player-play = Reproducir
player-pause = Pausar
player-now-playing = Sonando ahora
player-position = Posición en el documento
player-chapter-left = Quedan { $time } del capítulo
player-skip-back = Retroceder { $seconds } segundos
player-skip-forward = Avanzar { $seconds } segundos
player-error = La reproducción se detuvo de forma inesperada. Pulsa reproducir para intentarlo de nuevo.
contents-title = Índice
search-doc-title = Buscar en el texto
search-doc-placeholder = Busca una palabra o frase
search-doc-hint = Escribe al menos dos letras.
search-doc-count = { $count ->
    [one] Un resultado
   *[other] { $count } resultados
}
search-no-results = Sin resultados
keys-title = Atajos de teclado
keys-toggle = Reproducir o pausar
keys-skip = Retroceder o avanzar
keys-speed = Más lento o más rápido
keys-paragraph = Párrafo anterior o siguiente
keys-follow = Volver a la voz
keys-search = Buscar en el texto

## Speed and sleep
speed-title = Velocidad de reproducción
speed-slower = Más lento
speed-faster = Más rápido
speed-hint = El tono se mantiene natural a cualquier velocidad.
sleep-title = Temporizador
sleep-on = Temporizador activado
sleep-off = Desactivado
sleep-minutes = { $minutes } min
sleep-end-chapter = Al final del capítulo
sleep-stops-in = Se detiene en { $time }
sleep-hint = Los últimos diez segundos se apagan suavemente.
sleep-set = Temporizador: { $choice }
sleep-cleared = Temporizador desactivado
sleep-ended = El temporizador terminó. Que descanses.

## Voices
voices-title = Voces
voices-lead = Elige la voz que te lee. Toca el altavoz para escucharla antes.
voices-favourites = Favoritas
voices-recent = Recientes
voices-yours = Tus voces
voices-standard = Estándar
voices-standard-hint = Escucha ilimitada. Empiezan enseguida.
voices-expressive = Expresivas
voices-expressive-hint = Usan el saldo expresivo diario.
voices-expressive-line = Una interpretación más rica y emotiva. Cada vista previa y cada minuto de audio usan el saldo diario.
voices-budget-today = Saldo expresivo de hoy
voices-budget-numbers = { $left } de { $cap } s · { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }
voices-more = Más voces
voices-fewer = Menos voces
voice-title = Voz
voice-switched = Ahora lee { $name }
voice-default-set = { $name } es ahora tu voz predeterminada
voice-budget-left = Quedan hoy unos { $minutes } min de audio expresivo y { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }.
voice-badge-expressive = Expresiva
voice-favourite = Añadir a favoritas
voice-unfavourite = Quitar de favoritas
voice-preview = Escuchar { $name }
voice-manage = Gestionar voz
voice-manage-line = Al eliminarla se borran la voz y la grabación con la que se creó.
voice-delete = Eliminar esta voz
voice-deleted = Se eliminó { $name }.
voice-ask-title = ¿Reproducir una muestra expresiva?
voice-ask-line = Se genera una muestra corta en la GPU que usa unos { $seconds } segundos del saldo de hoy.
voice-ask-left = Quedan hoy { $left ->
        [one] { $left } segundo
       *[other] { $left } segundos
    } y { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }.
voice-ask-play = Reproducir muestra
voice-state-needs-recording = falta grabación
voice-state-preparing = preparando…
voice-state-failed = error
voice-yours-fallback = Tu voz

accent-american = Estadounidense
accent-british = Británico
gender-female = Femenina
gender-male = Masculina

## Recording a voice
record-title = Graba tu voz
record-cta = Graba tu voz
record-cta-line = Lee ocho frases cortas. Unos dos minutos.
record-tips = Busca un lugar tranquilo, sostén el teléfono a un palmo de la boca y lee cada frase en inglés con la emoción indicada.
record-name = Nombre
record-default-name = Mi voz
record-start = Grabar
record-stop = Detener
record-again = Grabar otra vez
record-progress = { $done } de { $total } frases grabadas
record-create = Crear voz
record-uploading = Subiendo tu grabación…
record-preparing = Preparando tu voz…
record-wait = Tarda uno o dos minutos. Puedes salir de esta pantalla; sigue sin ti.
record-ready = Tu voz está lista
record-ready-line = Lee con el motor expresivo, que usa el saldo diario.
record-use = Usar mi voz
record-failed-title = No ha funcionado
record-back = Volver a grabar
record-line-good = La frase { $line } suena bien.
record-failed = La grabación falló: { $detail }
record-every-line = Graba primero todas las frases.
record-prepare-failed = No se pudo convertir la grabación en una voz. Prueba de nuevo en un lugar más tranquilo.
record-vanished = La voz desapareció mientras se preparaba.
record-still-preparing = Aún se está preparando. Aparecerá en Tus voces cuando esté lista.
record-error-blocked = El micrófono está bloqueado. Permítelo en los ajustes del sitio del navegador y vuelve a intentarlo.
record-error-no-mic = No se encontró ningún micrófono.
record-error-other = No se pudo empezar a grabar: { $detail }
record-issue-clipping = Demasiado alto: se distorsionó. Aléjate un poco y vuelve a leerla.
record-issue-noisy = Hay ruido de fondo. Busca un sitio más tranquilo si puedes.
record-issue-quiet = Muy bajo. Acerca un poco el teléfono.
record-issue-short = Fue muy corto. Lee la frase completa.
mood-warm = Cálido
mood-calm = Tranquilo
mood-delighted = Encantado
mood-sorry = Arrepentido
mood-firm = Firme
mood-curious = Curioso
mood-amused = Divertido
mood-gentle = Suave

## Settings
settings-title = Ajustes
settings-account = Cuenta
settings-listening = Escucha
settings-appearance = Apariencia
settings-storage = Almacenamiento
settings-engine = Motor de voz
settings-about = Información
settings-default-voice = Voz predeterminada
settings-default-speed = Velocidad predeterminada
settings-skip-back = Retroceso
settings-skip-forward = Avance
settings-theme = Tema
settings-language = Idioma
theme-ink = Tinta
theme-paper = Papel
theme-night = Noche
theme-system = Sistema
language-auto = Igual que este dispositivo
language-auto-named = Automático ({ $language })
account-email = Correo
account-offline = Sin conexión
account-method-email = Código por correo
account-unlink = Desvincular
account-unlinked = Se quitó el método de acceso.
account-methods = Métodos de inicio de sesión
account-linked = Vinculado
account-not-linked = Sin vincular
account-link-short = Vincular
account-sign-out = Cerrar sesión
account-sign-out-all = Cerrar sesión en todos los dispositivos
account-sign-out-all-confirm = ¿Cerrar sesión en todos los dispositivos?
account-sign-out-all-line = Cada teléfono, tableta y ordenador con esta cuenta tendrá que iniciar sesión de nuevo.
account-sign-out-all-action = Cerrar en todos
storage-used = En uso en este dispositivo
storage-clear = Borrar audio guardado
storage-hint = El audio que ya escuchaste se guarda para reproducirlo sin conexión.
storage-clear-title = ¿Borrar el audio guardado?
storage-clear-line = Borrarlo libera espacio. Los fragmentos se descargan de nuevo cuando hagan falta.
storage-clear-confirm = Borrar
storage-cleared = Audio guardado borrado.
engine-voice = Estado
engine-awake = Activo
engine-asleep = En reposo (se activa al usarlo)
engine-starting = Arrancando
engine-unavailable = No disponible
engine-gpu-today = Saldo expresivo de hoy
engine-gpu-left = { $left } de { $cap } s · { $runs ->
        [one] { $runs } uso
       *[other] { $runs } usos
    }
engine-queue = En cola
engine-refresh = Actualizar estado
about-version = Versión
about-setup = Instalación y reproducción en segundo plano
about-source = Código fuente
about-colophon = Streams Reader · gratis y de código abierto

## Not found
not-found-title = Aquí no hay nada
not-found-line = Esa página no existe.
not-found-back = Volver a la biblioteca
