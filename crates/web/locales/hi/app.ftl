# Streams Reader, हिन्दी

app-name = Streams Reader

## Common actions
action-back = वापस
action-cancel = रद्द करें
action-close = बंद करें
action-delete = हटाएँ
action-dismiss = हटाएँ
action-not-now = अभी नहीं
action-retry = फिर से कोशिश करें
toast-reload = फिर से लोड करें
toast-undo = पहले जैसा करें
toast-update-ready = नया संस्करण तैयार है।

## Navigation
nav-label = मुख्य
nav-library = लाइब्रेरी
nav-voices = आवाज़ें
nav-settings = सेटिंग्स

## Times and numbers
time-left-under-minute = एक मिनट से कम बाकी
time-left-min = { $minutes } मिनट बाकी
time-left-hours = { $hours } घं { $minutes } मिनट बाकी
duration-min = { $minutes } मिनट
duration-hours = { $hours } घं { $minutes } मिनट
# Storage sizes (decimal units) and the playback speed; NUMBER() writes the language's decimal separator.
size-b = { NUMBER($n, maximumFractionDigits: 0) } B
size-kb = { NUMBER($n, maximumFractionDigits: 1) } KB
size-mb = { NUMBER($n, maximumFractionDigits: 1) } MB
size-gb = { NUMBER($n, maximumFractionDigits: 1) } GB
speed-value = { NUMBER($rate, maximumFractionDigits: 2) }×
seconds-short = { $seconds } सेकंड
wait-under-minute = एक मिनट से कम समय
wait-minutes = { $minutes ->
    [one] एक मिनट
   *[other] { $minutes } मिनट
}
wait-hours = { $hours ->
    [one] लगभग एक घंटे
   *[other] लगभग { $hours } घंटे
}

## Errors
error-offline = आप ऑफ़लाइन हैं। कनेक्शन जाँचें और फिर से कोशिश करें।
error-unexpected = सर्वर से अनपेक्षित जवाब आया। थोड़ी देर में फिर कोशिश करें।
error-browser = आपके ब्राउज़र ने मना कर दिया: { $detail }
error-bad-request = अनुरोध पूरी तरह सही नहीं था। फिर से कोशिश करें।
error-unauthorized = कृपया फिर से साइन इन करें।
error-forbidden = इसकी अनुमति नहीं है।
error-not-found = यह आइटम अब मौजूद नहीं है।
error-conflict = यह कहीं और बदल गया है। रीफ़्रेश करके फिर कोशिश करें।
error-too-large = यह अपलोड करने के लिए बहुत बड़ा है।
error-rate-limited = बहुत ज़्यादा कोशिशें। थोड़ा रुककर फिर कोशिश करें।
error-rate-limited-wait = बहुत ज़्यादा कोशिशें। { $wait } बाद फिर कोशिश करें।
error-gpu-budget = आज का एक्सप्रेसिव आवाज़ का समय खत्म हो गया है। कोई स्टैंडर्ड आवाज़ आज़माएँ।
error-gpu-budget-wait = आज का एक्सप्रेसिव आवाज़ का समय खत्म हो गया है। यह { $wait } बाद फिर मिलेगा।
error-engine-waking = वॉइस इंजन चालू हो रहा है। इसमें एक मिनट लग सकता है।
error-engine-failed = वॉइस इंजन वह हिस्सा नहीं पढ़ पाया।
error-upstream = पेज लोड नहीं हो सका। लिंक जाँचें या टेक्स्ट पेस्ट करें।
error-upgrade = नया संस्करण उपलब्ध है। फिर से लोड हो रहा है…
error-internal = सर्वर पर कुछ गड़बड़ हो गई। फिर से कोशिश करें।

## Sign in
sign-in-lead = आपकी पढ़ने की सूची, आपको पढ़कर सुनाई गई।
sign-in-title = Streams Reader में साइन इन करें
sign-in-email-label = ईमेल
sign-in-email-placeholder = aap@example.com
sign-in-email-invalid = यह ईमेल पता नहीं लगता।
sign-in-continue = ईमेल से जारी रखें
sign-in-or = या
sign-in-with = { $provider } से जारी रखें
sign-in-invite-label = आमंत्रण कोड
sign-in-invite-placeholder = जैसे STREAMS-1234
sign-in-invite-hint = अभी Streams Reader सिर्फ़ आमंत्रण से उपलब्ध है। जिसने आपको बुलाया है, उनसे कोड माँगें।
sign-in-code-title = अपना ईमेल देखें
sign-in-code-label = 6 अंकों का कोड
sign-in-verify = साइन इन करें
sign-in-link-hint = आप उस ईमेल का लिंक इसी डिवाइस पर भी खोल सकते हैं।
sign-in-resend = नया कोड भेजें
sign-in-resend-wait = { $seconds } सेकंड बाद फिर भेजें
sign-in-other-email = दूसरा ईमेल इस्तेमाल करें
sign-in-code-sent-before = हमने{" "}
sign-in-code-sent-after = {" "}पर 6 अंकों का कोड भेजा है।
sign-in-new-here = यहाँ नए हैं? खाता बनाने के लिए अपना आमंत्रण कोड डालें।
sign-in-have-invite = मेरे पास आमंत्रण कोड है
sign-in-oauth-invite = इस तरह खाता बनाने के लिए पहले अपना ईमेल और आमंत्रण कोड डालें।
sign-in-legal-before = जारी रखकर आप{" "}
sign-in-legal-and = {" "}और{" "}
sign-in-legal-after = {" "}से सहमत होते हैं।
legal-terms = शर्तों
legal-privacy = गोपनीयता नीति
auth-error-offline = साइन-इन सेवा तक नहीं पहुँच पाए। अपना कनेक्शन जाँचें।
auth-error-code = कोड गलत है या उसकी अवधि खत्म हो गई है। फिर कोशिश करें या नया कोड मँगाएँ।
auth-invite-needed = खाता बनाने के लिए आमंत्रण कोड चाहिए।
auth-error-invite = यह आमंत्रण कोड मान्य नहीं है या पहले ही इस्तेमाल हो चुका है।
auth-error-rate = बहुत ज़्यादा कोशिशें। एक मिनट रुककर फिर कोशिश करें।
auth-error-session = आप साइन आउट हो गए हैं। कृपया फिर से साइन इन करें।
auth-error-other = साइन इन नहीं हो पाया। थोड़ी देर में फिर कोशिश करें।
invite-used = यह आमंत्रण कोड पहले ही इस्तेमाल हो चुका है।
invite-expired = इस आमंत्रण कोड की अवधि खत्म हो गई है। नया कोड माँगें।
invite-taken = यह आमंत्रण कोड किसी दूसरे ईमेल से जुड़ा है।
invite-unknown = हम इस आमंत्रण कोड को नहीं पहचानते। जाँचकर फिर से कोशिश करें।
auth-callback-busy = साइन इन हो रहा है…
auth-callback-failed = साइन इन पूरा नहीं हुआ
auth-callback-back = साइन इन पर वापस जाएँ

## First run
welcome-title = Streams Reader में आपका स्वागत है
welcome-lead = दो छोटे काम, ताकि स्क्रीन बंद होने पर भी सुनना बिना रुके चले।
welcome-install-title = इसे होम स्क्रीन पर जोड़ें
welcome-install-line = यह पूरी स्क्रीन में खुलता है और मीडिया कंट्रोल में दिखता है।
welcome-install = इंस्टॉल करें
welcome-installed = इंस्टॉल हो गया
welcome-installed-toast = इंस्टॉल हो गया। इसे होम स्क्रीन से खोलें।
welcome-install-manual = Chrome में ⋮ मेन्यू खोलें और होम स्क्रीन में जोड़ें चुनें।
welcome-background-title = इसे बैकग्राउंड में चलने दें
welcome-background-line = स्क्रीन बंद होने के कुछ मिनट बाद Android ऑडियो रोक सकता है। Chrome को बैकग्राउंड में चलने की अनुमति दें:
welcome-android-1 = सेटिंग्स खोलें, फिर ऐप्लिकेशन, फिर Chrome।
welcome-android-2 = ऐप के लिए बैटरी का इस्तेमाल पर टैप करें।
welcome-android-3 = बिना पाबंदी वाला चुनें।
welcome-samsung-line = Samsung फ़ोन पर Chrome को स्लीप वाली सूचियों से भी हटाएँ:
welcome-samsung-1 = सेटिंग्स खोलें, फिर बैटरी, फिर बैकग्राउंड इस्तेमाल की सीमाएँ।
welcome-samsung-2 = Chrome को स्लीपिंग ऐप्स और डीप स्लीपिंग ऐप्स से हटाएँ।
welcome-keys-title = कीबोर्ड से सुनें
welcome-keys-line = रीडर में स्पेस से चलाएँ और रोकें, J और L से आगे-पीछे जाएँ, और [ व ] से गति बदलें।
welcome-start = सुनना शुरू करें

## Library
library-title = लाइब्रेरी
library-search = अपनी लाइब्रेरी में खोजें
library-search-placeholder = शीर्षक, लेखक या साइट
library-search-empty-line = कोई दूसरा शब्द आज़माएँ, या रीडर में किसी दस्तावेज़ के अंदर खोजें।
library-continue = सुनना जारी रखें
library-all = सभी दस्तावेज़
library-more = और विकल्प
library-play = चलाएँ
library-delete = लाइब्रेरी से हटाएँ
library-removed = “{ $title }” हटा दिया गया।
library-empty-title = अभी सुनने के लिए कुछ नहीं है
library-empty-line = कोई लेख, किताब या अपने नोट्स जोड़ें, फिर चलाएँ दबाएँ।
library-share-tip = सुझाव: ब्राउज़र से कोई भी पेज Streams Reader पर शेयर करें।
library-unreachable = आपकी लाइब्रेरी तक नहीं पहुँच पाए
library-unreachable-line = अपना कनेक्शन जाँचें, फिर से कोशिश करें।
library-preparing = तैयार हो रहा है…
library-upload-failed = अपलोड पूरा नहीं हुआ
library-retry-gone = यह फ़ाइल अब इस डिवाइस पर नहीं है। इसे हटाएँ, फिर दोबारा जोड़ें।
doc-finished = पूरा हुआ
source-web = वेब पेज
source-text = टेक्स्ट
source-book = किताब
source-pdf = PDF
source-text-file = टेक्स्ट फ़ाइल
source-markdown = Markdown

## Adding documents
add-title = लाइब्रेरी में जोड़ें
add-paste = टेक्स्ट पेस्ट करें
add-paste-hint = कोई लेख, अध्याय, कुछ भी
add-link = लिंक पेस्ट करें
add-link-hint = लेख और ब्लॉग पोस्ट सबसे अच्छे चलते हैं
add-file = फ़ाइल खोलें
add-file-hint = EPUB, PDF, TXT, Markdown या HTML
add-paste-title-label = शीर्षक (वैकल्पिक)
add-paste-text-label = टेक्स्ट
add-link-label = वेब पता
add-paste-placeholder = टेक्स्ट यहाँ पेस्ट करें या लिखें।
add-paste-submit = टेक्स्ट जोड़ें
add-paste-empty = पहले कुछ टेक्स्ट पेस्ट करें।
add-link-help = पेज को साफ़ किया जाता है ताकि सिर्फ़ मुख्य सामग्री पढ़ी जाए।
add-link-submit = लिंक जोड़ें
add-link-invalid = यह वेब पता नहीं लगता।
add-file-error = वह फ़ाइल नहीं खुल सकी: { $detail }
add-share-tip = आप दूसरे ऐप्स से भी कोई पेज या फ़ाइल Streams Reader पर शेयर कर सकते हैं।
add-done = आपकी लाइब्रेरी में जोड़ दिया गया।
import-stage-fetching = पेज लाया जा रहा है…
import-stage-reading = पढ़ा जा रहा है…
import-stage-uploading = लाइब्रेरी में जोड़ा जा रहा है…
import-error-empty = इसमें पढ़ने लायक टेक्स्ट नहीं है।
import-error-too-large = यह बहुत बड़ा है ({ $detail })।
import-error-damaged = फ़ाइल खराब लगती है और पढ़ी नहीं जा सकी।
import-error-unsupported = समर्थित नहीं: { $detail }।
import-error-pdf = वह PDF पढ़ा नहीं जा सका ({ $detail })।
import-error-file-type = यह फ़ाइल प्रकार समर्थित नहीं है। EPUB, PDF, TXT, Markdown या HTML आज़माएँ।
import-error-worker = इम्पोर्टर शुरू नहीं हुआ। पेज रीलोड करके फिर से कोशिश करें।

## Reader and player
reader-back = लाइब्रेरी पर वापस जाएँ
reader-more = और
reader-play-here = यहाँ से चलाएँ
reader-back-to-voice = आवाज़ पर लौटें
reader-end = समाप्त
reader-no-text = इस दस्तावेज़ में पढ़ने लायक टेक्स्ट नहीं है।
reader-hide-text = बिना टेक्स्ट के सुनें
reader-hide-text-hint = चलते समय सिर्फ़ कवर दिखता है
reader-show-text = टेक्स्ट दिखाएँ
reader-text-size = टेक्स्ट का आकार
bookmark-add = इस जगह को बुकमार्क करें
bookmark-added = बुकमार्क सहेजा गया।
bookmark-exists = यह जगह पहले से बुकमार्क है।
bookmark-remove = बुकमार्क हटाएँ
reader-text-smaller = छोटा टेक्स्ट
reader-text-larger = बड़ा टेक्स्ट
player-play = चलाएँ
player-pause = रोकें
player-now-playing = अभी चल रहा है
player-position = दस्तावेज़ में जगह
player-chapter-left = अध्याय में { $time } बाकी
player-skip-back = { $seconds } सेकंड पीछे
player-skip-forward = { $seconds } सेकंड आगे
player-error = प्लेबैक अचानक रुक गया। फिर से कोशिश करने के लिए चलाएँ दबाएँ।
contents-title = विषय-सूची
search-doc-title = टेक्स्ट में खोजें
search-doc-placeholder = कोई शब्द या वाक्यांश खोजें
search-doc-hint = कम से कम दो अक्षर लिखें।
search-doc-count = { $count ->
    [one] एक नतीजा
   *[other] { $count } नतीजे
}
search-no-results = कोई नतीजा नहीं
keys-title = कीबोर्ड शॉर्टकट
keys-toggle = चलाएँ या रोकें
keys-skip = पीछे या आगे जाएँ
keys-speed = धीमा या तेज़
keys-paragraph = पिछला या अगला अनुच्छेद
keys-follow = आवाज़ पर लौटें
keys-search = टेक्स्ट में खोजें

## Speed and sleep
speed-title = चलाने की गति
speed-slower = धीमा
speed-faster = तेज़
speed-hint = हर गति पर आवाज़ का सुर स्वाभाविक रहता है।
sleep-title = स्लीप टाइमर
sleep-on = स्लीप टाइमर चालू है
sleep-off = बंद
sleep-minutes = { $minutes } मिनट
sleep-end-chapter = अध्याय के अंत पर
sleep-stops-in = { $time } में रुकेगा
sleep-hint = आख़िरी दस सेकंड धीरे-धीरे धीमे होते हैं।
sleep-set = स्लीप टाइमर: { $choice }
sleep-cleared = स्लीप टाइमर बंद
sleep-ended = स्लीप टाइमर खत्म हुआ। शुभ रात्रि।

## Voices
voices-title = आवाज़ें
voices-lead = वह आवाज़ चुनें जो आपको पढ़कर सुनाए। पहले सुनने के लिए स्पीकर पर टैप करें।
voices-favourites = पसंदीदा
voices-recent = हाल की
voices-yours = आपकी आवाज़ें
voices-standard = स्टैंडर्ड
voices-standard-hint = असीमित सुनना। जल्दी शुरू होती हैं।
voices-expressive = एक्सप्रेसिव
voices-expressive-hint = रोज़ के एक्सप्रेसिव बजट से चलती है।
voices-expressive-line = ज़्यादा भावपूर्ण और समृद्ध अंदाज़। हर प्रीव्यू और ऑडियो का हर मिनट रोज़ के बजट से कटता है।
voices-budget-today = आज का एक्सप्रेसिव बजट
voices-budget-numbers = { $cap } में से { $left } सेकंड · { $runs ->
        [one] { $runs } बार
       *[other] { $runs } बार
    }
voices-more = और आवाज़ें
voices-fewer = कम आवाज़ें
voice-title = आवाज़
voice-switched = अब { $name } पढ़ रही है
voice-default-set = { $name } अब आपकी डिफ़ॉल्ट आवाज़ है
voice-budget-left = आज लगभग { $minutes } मिनट का एक्सप्रेसिव ऑडियो और { $runs ->
        [one] { $runs } बार
       *[other] { $runs } बार
    } बाकी हैं।
voice-badge-expressive = एक्सप्रेसिव
voice-favourite = पसंदीदा में जोड़ें
voice-unfavourite = पसंदीदा से हटाएँ
voice-preview = { $name } सुनें
voice-manage = आवाज़ प्रबंधित करें
voice-manage-line = हटाने पर आवाज़ और उसे बनाने वाली रिकॉर्डिंग दोनों मिट जाती हैं।
voice-delete = यह आवाज़ हटाएँ
voice-deleted = { $name } हटा दी गई।
voice-ask-title = एक्सप्रेसिव प्रीव्यू चलाएँ?
voice-ask-line = इससे GPU पर एक छोटा नमूना बनता है और आज के बजट के लगभग { $seconds } सेकंड लगते हैं।
voice-ask-left = आज { $left ->
        [one] { $left } सेकंड
       *[other] { $left } सेकंड
    } और { $runs ->
        [one] { $runs } बार
       *[other] { $runs } बार
    } बाकी हैं।
voice-ask-play = प्रीव्यू चलाएँ
voice-state-needs-recording = रिकॉर्डिंग चाहिए
voice-state-preparing = तैयार हो रही है…
voice-state-failed = विफल
voice-yours-fallback = आपकी आवाज़

accent-american = अमेरिकी
accent-british = ब्रिटिश
gender-female = महिला
gender-male = पुरुष

## Recording a voice
record-title = अपनी आवाज़ रिकॉर्ड करें
record-cta = अपनी आवाज़ रिकॉर्ड करें
record-cta-line = आठ छोटी पंक्तियाँ पढ़ें। लगभग दो मिनट।
record-tips = किसी शांत कमरे में बैठें, फ़ोन को मुँह से एक हथेली की दूरी पर रखें, और हर अंग्रेज़ी पंक्ति को बताए गए भाव से पढ़ें।
record-name = नाम
record-default-name = मेरी आवाज़
record-start = रिकॉर्ड करें
record-stop = रोकें
record-again = फिर से रिकॉर्ड करें
record-progress = { $total } में से { $done } पंक्तियाँ रिकॉर्ड हुईं
record-create = आवाज़ बनाएँ
record-uploading = आपकी रिकॉर्डिंग अपलोड हो रही है…
record-preparing = आपकी आवाज़ तैयार हो रही है…
record-wait = इसमें एक-दो मिनट लगते हैं। आप यह स्क्रीन छोड़ सकते हैं; काम चलता रहेगा।
record-ready = आपकी आवाज़ तैयार है
record-ready-line = यह एक्सप्रेसिव इंजन से पढ़ती है, जो रोज़ के बजट से चलता है।
record-use = मेरी आवाज़ इस्तेमाल करें
record-failed-title = यह नहीं हो पाया
record-back = रिकॉर्डिंग पर वापस जाएँ
record-line-good = पंक्ति { $line } अच्छी लग रही है।
record-failed = रिकॉर्डिंग विफल रही: { $detail }
record-every-line = पहले सभी पंक्तियाँ रिकॉर्ड करें।
record-prepare-failed = रिकॉर्डिंग से आवाज़ नहीं बन सकी। किसी शांत जगह पर फिर कोशिश करें।
record-vanished = तैयार होते समय आवाज़ गायब हो गई।
record-still-preparing = अभी तैयार हो रही है। तैयार होने पर यह आपकी आवाज़ें में दिखेगी।
record-error-blocked = माइक्रोफ़ोन की अनुमति बंद है। ब्राउज़र की साइट सेटिंग्स में इसे चालू करके फिर कोशिश करें।
record-error-no-mic = कोई माइक्रोफ़ोन नहीं मिला।
record-error-other = रिकॉर्डिंग शुरू नहीं हो सकी: { $detail }
record-issue-clipping = बहुत तेज़, आवाज़ फट गई। थोड़ा पीछे होकर फिर पढ़ें।
record-issue-noisy = पीछे शोर है। हो सके तो किसी शांत जगह जाएँ।
record-issue-quiet = बहुत धीमी। फ़ोन थोड़ा पास रखें।
record-issue-short = यह बहुत छोटा था। पूरी पंक्ति पढ़ें।
mood-warm = आत्मीयता से
mood-calm = शांति से
mood-delighted = खुशी से
mood-sorry = अफ़सोस से
mood-firm = दृढ़ता से
mood-curious = जिज्ञासा से
mood-amused = मज़ाक में
mood-gentle = कोमलता से

## Settings
settings-title = सेटिंग्स
settings-account = खाता
settings-listening = सुनना
settings-appearance = रूप
settings-storage = स्टोरेज
settings-engine = वॉइस इंजन
settings-about = जानकारी
settings-default-voice = डिफ़ॉल्ट आवाज़
settings-default-speed = डिफ़ॉल्ट गति
settings-skip-back = पीछे जाने की अवधि
settings-skip-forward = आगे जाने की अवधि
settings-theme = थीम
settings-language = भाषा
theme-ink = स्याही
theme-paper = कागज़
theme-night = रात
theme-system = सिस्टम
language-auto = इस डिवाइस जैसी
language-auto-named = अपने-आप ({ $language })
account-email = ईमेल
account-offline = ऑफ़लाइन
account-method-email = ईमेल कोड
account-unlink = अलग करें
account-unlinked = साइन-इन का तरीका हटा दिया गया।
account-methods = साइन-इन के तरीके
account-linked = जुड़ा हुआ
account-not-linked = जुड़ा नहीं
account-link-short = जोड़ें
account-sign-out = साइन आउट करें
account-sign-out-all = सभी डिवाइस से साइन आउट करें
account-sign-out-all-confirm = सभी डिवाइस से साइन आउट करें?
account-sign-out-all-line = इस खाते से साइन इन हर फ़ोन, टैबलेट और कंप्यूटर को फिर से साइन इन करना होगा।
account-sign-out-all-action = हर जगह साइन आउट करें
storage-used = इस डिवाइस पर इस्तेमाल
storage-clear = सहेजा ऑडियो साफ़ करें
storage-hint = जो ऑडियो आप सुन चुके हैं, वह ऑफ़लाइन चलाने के लिए सहेजा रहता है।
storage-clear-title = सहेजा ऑडियो साफ़ करें?
storage-clear-line = इससे जगह खाली होती है। ज़रूरत पड़ने पर क्लिप फिर से डाउनलोड होंगी।
storage-clear-confirm = साफ़ करें
storage-cleared = सहेजा ऑडियो साफ़ हो गया।
engine-voice = स्थिति
engine-awake = चालू
engine-asleep = सोया हुआ (पहली बार इस्तेमाल पर जागता है)
engine-starting = शुरू हो रहा है
engine-unavailable = उपलब्ध नहीं
engine-gpu-today = आज का एक्सप्रेसिव बजट
engine-gpu-left = { $cap } में से { $left } सेकंड · { $runs ->
        [one] { $runs } बार
       *[other] { $runs } बार
    }
engine-queue = बनने की कतार में
engine-refresh = स्थिति रीफ़्रेश करें
about-version = संस्करण
about-setup = इंस्टॉल और बैकग्राउंड प्लेबैक
about-source = सोर्स कोड
about-colophon = Streams Reader · मुफ़्त और ओपन सोर्स

## Not found
not-found-title = यहाँ कुछ नहीं है
not-found-line = वह पेज मौजूद नहीं है।
not-found-back = लाइब्रेरी पर वापस जाएँ
