# Streams Reader, français

app-name = Streams Reader

## Common actions
action-back = Retour
action-cancel = Annuler
action-close = Fermer
action-delete = Supprimer
action-dismiss = Ignorer
action-not-now = Pas maintenant
action-retry = Réessayer
toast-reload = Recharger
toast-undo = Annuler
toast-update-ready = Une nouvelle version est prête.

## Navigation
nav-label = Principal
nav-library = Bibliothèque
nav-voices = Voix
nav-settings = Réglages

## Times and numbers
time-left-under-minute = Moins d'une minute restante
time-left-min = { $minutes } min restantes
time-left-hours = { $hours } h { $minutes } min restantes
duration-min = { $minutes } min
duration-hours = { $hours } h { $minutes } min
# Storage sizes (decimal units) and the playback speed; NUMBER() writes the language's decimal separator.
size-b = { NUMBER($n, maximumFractionDigits: 0) } o
size-kb = { NUMBER($n, maximumFractionDigits: 1) } ko
size-mb = { NUMBER($n, maximumFractionDigits: 1) } Mo
size-gb = { NUMBER($n, maximumFractionDigits: 1) } Go
speed-value = { NUMBER($rate, maximumFractionDigits: 2) }×
seconds-short = { $seconds } s
wait-under-minute = moins d'une minute
wait-minutes = { $minutes ->
    [one] une minute
   *[other] { $minutes } minutes
}
wait-hours = { $hours ->
    [one] environ une heure
   *[other] environ { $hours } heures
}

## Errors
error-offline = Vous êtes hors ligne. Vérifiez votre connexion et réessayez.
error-unexpected = Le serveur a renvoyé une réponse inattendue. Réessayez dans un instant.
error-browser = Votre navigateur a refusé : { $detail }
error-bad-request = La demande n'était pas tout à fait correcte. Réessayez.
error-unauthorized = Veuillez vous reconnecter.
error-forbidden = Ce n'est pas autorisé.
error-not-found = Cet élément n'existe plus.
error-conflict = Cela a changé ailleurs. Actualisez et réessayez.
error-too-large = C'est trop volumineux pour être envoyé.
error-rate-limited = Trop de tentatives. Patientez un peu, puis réessayez.
error-rate-limited-wait = Trop de tentatives. Réessayez dans { $wait }.
error-gpu-budget = Le temps de voix expressives du jour est épuisé. Essayez une voix standard.
error-gpu-budget-wait = Le temps de voix expressives du jour est épuisé. Il se renouvelle dans { $wait }.
error-engine-waking = Le moteur vocal se réveille. Cela peut prendre une minute.
error-engine-failed = Le moteur vocal n'a pas pu lire ce passage.
error-upstream = Impossible de charger la page. Vérifiez le lien ou collez plutôt le texte.
error-upgrade = Une nouvelle version est disponible. Rechargement…
error-internal = Un problème est survenu sur le serveur. Réessayez.

## Sign in
sign-in-lead = Votre liste de lecture, lue à voix haute.
sign-in-title = Connectez-vous à Streams Reader
sign-in-email-label = E-mail
sign-in-email-placeholder = vous@exemple.fr
sign-in-email-invalid = Cela ne ressemble pas à une adresse e-mail.
sign-in-continue = Continuer avec l'e-mail
sign-in-or = ou
sign-in-with = Continuer avec { $provider }
sign-in-invite-label = Code d'invitation
sign-in-invite-placeholder = Par exemple STREAMS-1234
sign-in-invite-hint = Streams Reader est pour l'instant sur invitation. Demandez le code à la personne qui vous a invité.
sign-in-code-title = Consultez vos e-mails
sign-in-code-label = Code à 6 chiffres
sign-in-verify = Se connecter
sign-in-link-hint = Vous pouvez aussi ouvrir le lien de cet e-mail sur cet appareil.
sign-in-resend = Envoyer un nouveau code
sign-in-resend-wait = Renvoyer dans { $seconds } s
sign-in-other-email = Utiliser une autre adresse
sign-in-code-sent-before = Nous avons envoyé un code à 6 chiffres à{" "}
sign-in-code-sent-after = .
sign-in-new-here = Nouveau ici ? Ajoutez votre code d'invitation pour créer votre compte.
sign-in-have-invite = J'ai un code d'invitation
sign-in-oauth-invite = Pour créer un compte de cette façon, saisissez d'abord votre e-mail et votre code d'invitation.
sign-in-legal-before = En continuant, vous acceptez les{" "}
sign-in-legal-and = {" "}et la{" "}
sign-in-legal-after = .
legal-terms = Conditions
legal-privacy = Confidentialité
auth-error-offline = Impossible de joindre le service de connexion. Vérifiez votre connexion.
auth-error-code = Ce code est incorrect ou a expiré. Réessayez ou demandez-en un nouveau.
auth-invite-needed = Il faut un code d'invitation pour créer un compte.
auth-error-invite = Ce code d'invitation n'est pas valide ou a déjà servi.
auth-error-rate = Trop d'essais. Patientez une minute, puis réessayez.
auth-error-session = Vous avez été déconnecté. Veuillez vous reconnecter.
auth-error-other = La connexion n'a pas abouti. Réessayez dans un instant.
invite-used = Ce code d'invitation a déjà été utilisé.
invite-expired = Ce code d'invitation a expiré. Demandez-en un nouveau.
invite-taken = Ce code d'invitation est lié à une autre adresse e-mail.
invite-unknown = Nous ne reconnaissons pas ce code d'invitation. Vérifiez-le et réessayez.
auth-callback-busy = Connexion en cours…
auth-callback-failed = La connexion n'a pas abouti
auth-callback-back = Retour à la connexion

## First run
welcome-title = Bienvenue dans Streams Reader
welcome-lead = Deux réglages rapides pour une écoute sans coupure, même écran éteint.
welcome-install-title = Ajoutez-le à l'écran d'accueil
welcome-install-line = Il s'ouvre en plein écran et apparaît dans les commandes multimédias.
welcome-install = Installer
welcome-installed = Installé
welcome-installed-toast = Installé. Ouvrez-le depuis l'écran d'accueil.
welcome-install-manual = Dans Chrome, ouvrez le menu ⋮ et choisissez Ajouter à l'écran d'accueil.
welcome-background-title = Laissez-le jouer en arrière-plan
welcome-background-line = Android peut couper le son quelques minutes après l'extinction de l'écran. Autorisez Chrome en arrière-plan :
welcome-android-1 = Ouvrez Paramètres, puis Applications, puis Chrome.
welcome-android-2 = Touchez Utilisation de la batterie par l'application.
welcome-android-3 = Choisissez Sans restriction.
welcome-samsung-line = Sur les téléphones Samsung, retirez aussi Chrome des listes de mise en veille :
welcome-samsung-1 = Ouvrez Paramètres, puis Batterie, puis Limites d'utilisation en arrière-plan.
welcome-samsung-2 = Retirez Chrome des Applications en veille et des Applications en veille prolongée.
welcome-keys-title = Écoutez au clavier
welcome-keys-line = Dans le lecteur, Espace lance et met en pause, J et L reculent et avancent, [ et ] changent la vitesse.
welcome-start = Commencer l'écoute

## Library
library-title = Bibliothèque
library-search = Rechercher dans votre bibliothèque
library-search-placeholder = Titre, auteur ou site
library-search-empty-line = Essayez un autre mot, ou cherchez dans un document depuis le lecteur.
library-continue = Reprendre l'écoute
library-all = Tous les documents
library-more = Plus d'actions
library-play = Écouter
library-delete = Retirer de la bibliothèque
library-removed = « { $title } » a été retiré.
library-empty-title = Rien à écouter pour l'instant
library-empty-line = Ajoutez un article, un livre ou vos propres notes, puis lancez la lecture.
library-share-tip = Astuce : partagez n'importe quelle page de votre navigateur vers Streams Reader.
library-unreachable = Impossible d'accéder à votre bibliothèque
library-unreachable-line = Vérifiez votre connexion, puis réessayez.
library-preparing = Préparation…
library-upload-failed = L’envoi n’a pas abouti
library-retry-gone = Ce fichier n’est plus sur cet appareil. Supprimez-le, puis ajoutez-le à nouveau.
doc-finished = Terminé
source-web = Page web
source-text = Texte
source-book = Livre
source-pdf = PDF
source-text-file = Fichier texte
source-markdown = Markdown

## Adding documents
add-title = Ajouter à la bibliothèque
add-paste = Coller du texte
add-paste-hint = Un article, un chapitre, tout ce que vous voulez
add-link = Coller un lien
add-link-hint = Idéal pour les articles et les blogs
add-file = Ouvrir un fichier
add-file-hint = EPUB, PDF, TXT, Markdown ou HTML
add-paste-title-label = Titre (facultatif)
add-paste-text-label = Texte
add-link-label = Adresse web
add-paste-placeholder = Collez ou saisissez le texte ici.
add-paste-submit = Ajouter le texte
add-paste-empty = Collez d'abord un peu de texte.
add-link-help = La page est nettoyée pour ne lire que le contenu.
add-link-submit = Ajouter le lien
add-link-invalid = Cela ne ressemble pas à une adresse web.
add-file-error = Impossible d'ouvrir ce fichier : { $detail }
add-share-tip = Vous pouvez aussi partager une page ou un fichier vers Streams Reader depuis d'autres applis.
add-done = Ajouté à votre bibliothèque.
import-stage-fetching = Récupération de la page…
import-stage-reading = Lecture…
import-stage-uploading = Ajout à votre bibliothèque…
import-error-empty = Il n'y a aucun texte lisible.
import-error-too-large = C'est trop volumineux ({ $detail }).
import-error-damaged = Ce fichier semble endommagé et n'a pas pu être lu.
import-error-unsupported = Non pris en charge : { $detail }.
import-error-pdf = Impossible de lire ce PDF ({ $detail }).
import-error-file-type = Ce type de fichier n'est pas pris en charge. Essayez EPUB, PDF, TXT, Markdown ou HTML.
import-error-worker = L'importateur n'a pas démarré. Rechargez la page et réessayez.

## Reader and player
reader-back = Retour à la bibliothèque
reader-more = Plus
reader-play-here = Écouter à partir d'ici
reader-back-to-voice = Revenir à la voix
reader-end = Fin
reader-no-text = Ce document ne contient aucun texte lisible.
reader-hide-text = Écouter sans le texte
reader-hide-text-hint = Seule la couverture s'affiche pendant l'écoute
reader-show-text = Afficher le texte
reader-text-size = Taille du texte
bookmark-add = Marquer cet endroit
bookmark-added = Signet enregistré.
bookmark-exists = Ce passage a déjà un signet.
bookmark-remove = Supprimer le signet
reader-text-smaller = Texte plus petit
reader-text-larger = Texte plus grand
player-play = Lecture
player-pause = Pause
player-now-playing = En cours d'écoute
player-position = Position dans le document
player-chapter-left = { $time } restantes dans le chapitre
player-skip-back = Reculer de { $seconds } secondes
player-skip-forward = Avancer de { $seconds } secondes
player-error = La lecture s'est arrêtée de façon inattendue. Appuyez sur lecture pour réessayer.
contents-title = Sommaire
search-doc-title = Rechercher dans le texte
search-doc-placeholder = Un mot ou une expression
search-doc-hint = Saisissez au moins deux lettres.
search-doc-count = { $count ->
    [one] Un résultat
   *[other] { $count } résultats
}
search-no-results = Aucun résultat
keys-title = Raccourcis clavier
keys-toggle = Lecture ou pause
keys-skip = Reculer ou avancer
keys-speed = Plus lent ou plus rapide
keys-paragraph = Paragraphe précédent ou suivant
keys-follow = Revenir à la voix
keys-search = Rechercher dans le texte

## Speed and sleep
speed-title = Vitesse de lecture
speed-slower = Plus lent
speed-faster = Plus rapide
speed-hint = La voix garde un timbre naturel à toutes les vitesses.
sleep-title = Minuteur de sommeil
sleep-on = Minuteur activé
sleep-off = Désactivé
sleep-minutes = { $minutes } min
sleep-end-chapter = Fin du chapitre
sleep-stops-in = Arrêt dans { $time }
sleep-hint = Les dix dernières secondes s'estompent en douceur.
sleep-set = Minuteur : { $choice }
sleep-cleared = Minuteur désactivé
sleep-ended = Le minuteur est terminé. Bonne nuit.

## Voices
voices-title = Voix
voices-lead = Choisissez la voix qui vous lit. Touchez le haut-parleur pour l'écouter d'abord.
voices-favourites = Favorites
voices-recent = Récentes
voices-yours = Vos voix
voices-standard = Standard
voices-standard-hint = Écoute illimitée. Démarrage rapide.
voices-expressive = Expressives
voices-expressive-hint = Utilise le crédit expressif quotidien.
voices-expressive-line = Une interprétation plus riche et plus émouvante. Chaque aperçu et chaque minute d'audio puisent dans le crédit du jour.
voices-budget-today = Crédit expressif du jour
voices-budget-numbers = { $left } sur { $cap } s · { $runs ->
        [one] { $runs } essai
       *[other] { $runs } essais
    }
voices-more = Plus de voix
voices-fewer = Moins de voix
voice-title = Voix
voice-switched = Lecture avec { $name }
voice-default-set = { $name } est désormais votre voix par défaut
voice-budget-left = Il reste aujourd'hui environ { $minutes } min d'audio expressif et { $runs ->
        [one] { $runs } essai
       *[other] { $runs } essais
    }.
voice-badge-expressive = Expressive
voice-favourite = Ajouter aux favorites
voice-unfavourite = Retirer des favorites
voice-preview = Écouter { $name }
voice-manage = Gérer la voix
voice-manage-line = La suppression efface la voix et l'enregistrement qui a servi à la créer.
voice-delete = Supprimer cette voix
voice-deleted = { $name } a été supprimée.
voice-ask-title = Écouter un aperçu expressif ?
voice-ask-line = Un court extrait est généré sur le GPU, ce qui utilise environ { $seconds } secondes du crédit du jour.
voice-ask-left = Il reste aujourd'hui { $left ->
        [one] { $left } seconde
       *[other] { $left } secondes
    } et { $runs ->
        [one] { $runs } essai
       *[other] { $runs } essais
    }.
voice-ask-play = Écouter l'aperçu
voice-state-needs-recording = enregistrement requis
voice-state-preparing = préparation…
voice-state-failed = échec
voice-yours-fallback = Votre voix

accent-american = Américain
accent-british = Britannique
gender-female = Féminine
gender-male = Masculine

## Recording a voice
record-title = Enregistrez votre voix
record-cta = Enregistrez votre voix
record-cta-line = Lisez huit phrases courtes. Environ deux minutes.
record-tips = Installez-vous au calme, tenez le téléphone à une main de votre bouche et lisez chaque phrase anglaise avec l'émotion indiquée.
record-name = Nom
record-default-name = Ma voix
record-start = Enregistrer
record-stop = Arrêter
record-again = Recommencer
record-progress = { $done } phrases sur { $total } enregistrées
record-create = Créer la voix
record-uploading = Envoi de votre enregistrement…
record-preparing = Préparation de votre voix…
record-wait = Cela prend une à deux minutes. Vous pouvez quitter cet écran, la préparation continue.
record-ready = Votre voix est prête
record-ready-line = Elle lit avec le moteur expressif, qui puise dans le crédit quotidien.
record-use = Utiliser ma voix
record-failed-title = Cela n'a pas fonctionné
record-back = Retour à l'enregistrement
record-line-good = La phrase { $line } sonne bien.
record-failed = L'enregistrement a échoué : { $detail }
record-every-line = Enregistrez d'abord toutes les phrases.
record-prepare-failed = L'enregistrement n'a pas pu devenir une voix. Réessayez dans un endroit plus calme.
record-vanished = La voix a disparu pendant sa préparation.
record-still-preparing = Préparation en cours. Elle apparaîtra dans Vos voix une fois prête.
record-error-blocked = L'accès au micro est bloqué. Autorisez-le dans les réglages du site, puis réessayez.
record-error-no-mic = Aucun micro détecté.
record-error-other = Impossible de démarrer l'enregistrement : { $detail }
record-issue-clipping = Trop fort : le son sature. Éloignez-vous un peu et relisez.
record-issue-noisy = Il y a du bruit de fond. Trouvez un endroit plus calme si possible.
record-issue-quiet = Très faible. Rapprochez un peu le téléphone.
record-issue-short = C'était très court. Lisez la phrase en entier.
mood-warm = Chaleureux
mood-calm = Calme
mood-delighted = Ravi
mood-sorry = Désolé
mood-firm = Ferme
mood-curious = Curieux
mood-amused = Amusé
mood-gentle = Doux

## Settings
settings-title = Réglages
settings-account = Compte
settings-listening = Écoute
settings-appearance = Apparence
settings-storage = Stockage
settings-engine = Moteur vocal
settings-about = À propos
settings-default-voice = Voix par défaut
settings-default-speed = Vitesse par défaut
settings-skip-back = Recul
settings-skip-forward = Avance
settings-theme = Thème
settings-language = Langue
theme-ink = Encre
theme-paper = Papier
theme-night = Nuit
theme-system = Système
language-auto = Comme cet appareil
language-auto-named = Automatique ({ $language })
account-email = E-mail
account-offline = Hors ligne
account-method-email = Code par e-mail
account-unlink = Dissocier
account-unlinked = Méthode de connexion retirée.
account-methods = Méthodes de connexion
account-linked = Associé
account-not-linked = Non associé
account-link-short = Associer
account-sign-out = Se déconnecter
account-sign-out-all = Se déconnecter de tous les appareils
account-sign-out-all-confirm = Se déconnecter de tous les appareils ?
account-sign-out-all-line = Chaque téléphone, tablette et ordinateur connecté à ce compte devra se reconnecter.
account-sign-out-all-action = Tout déconnecter
storage-used = Utilisé sur cet appareil
storage-clear = Effacer l'audio enregistré
storage-hint = L'audio déjà écouté est conservé pour être réécouté hors ligne.
storage-clear-title = Effacer l'audio enregistré ?
storage-clear-line = Cela libère de l'espace. Les extraits seront retéléchargés au besoin.
storage-clear-confirm = Effacer
storage-cleared = Audio enregistré effacé.
engine-voice = État
engine-awake = Actif
engine-asleep = En veille (se réveille au premier usage)
engine-starting = Démarrage
engine-unavailable = Indisponible
engine-gpu-today = Crédit expressif du jour
engine-gpu-left = { $left } sur { $cap } s · { $runs ->
        [one] { $runs } essai
       *[other] { $runs } essais
    }
engine-queue = En attente
engine-refresh = Actualiser l'état
about-version = Version
about-setup = Installation et lecture en arrière-plan
about-source = Code source
about-colophon = Streams Reader · gratuit et open source

## Not found
not-found-title = Rien ici
not-found-line = Cette page n'existe pas.
not-found-back = Retour à la bibliothèque
