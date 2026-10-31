# Streams Reader, العربية

app-name = Streams Reader

## Common actions
action-back = رجوع
action-cancel = إلغاء
action-close = إغلاق
action-delete = حذف
action-dismiss = تجاهل
action-not-now = ليس الآن
action-retry = حاول مرة أخرى
toast-reload = إعادة التحميل
toast-undo = تراجع
toast-update-ready = إصدار جديد جاهز.

## Navigation
nav-label = التنقل الرئيسي
nav-library = المكتبة
nav-voices = الأصوات
nav-settings = الإعدادات

## Times and numbers
time-left-under-minute = أقل من دقيقة متبقية
time-left-min = متبقٍّ { $minutes } د
time-left-hours = متبقٍّ { $hours } س و{ $minutes } د
duration-min = { $minutes } د
duration-hours = { $hours } س و{ $minutes } د
# Storage sizes (decimal units) and the playback speed; NUMBER() writes the language's decimal separator.
size-b = { NUMBER($n, maximumFractionDigits: 0) } بايت
size-kb = { NUMBER($n, maximumFractionDigits: 1) } كيلوبايت
size-mb = { NUMBER($n, maximumFractionDigits: 1) } ميغابايت
size-gb = { NUMBER($n, maximumFractionDigits: 1) } غيغابايت
speed-value = { NUMBER($rate, maximumFractionDigits: 2) }×
seconds-short = { $seconds } ث
wait-under-minute = أقل من دقيقة
wait-minutes = { $minutes ->
    [one] دقيقة واحدة
    [two] دقيقتين
    [few] { $minutes } دقائق
   *[other] { $minutes } دقيقة
}
wait-hours = { $hours ->
    [one] نحو ساعة
    [two] نحو ساعتين
    [few] نحو { $hours } ساعات
   *[other] نحو { $hours } ساعة
}

## Errors
error-offline = أنت غير متصل بالإنترنت. تحقّق من الاتصال وحاول مرة أخرى.
error-unexpected = وصل من الخادم رد غير متوقع. حاول بعد قليل.
error-browser = رفض المتصفح الطلب: { $detail }
error-bad-request = لم يكن الطلب صحيحًا تمامًا. حاول مرة أخرى.
error-unauthorized = يُرجى تسجيل الدخول من جديد.
error-forbidden = هذا الإجراء غير مسموح.
error-not-found = هذا العنصر لم يعد موجودًا.
error-conflict = تغيّر هذا في مكان آخر. حدّث الصفحة وحاول مرة أخرى.
error-too-large = الحجم أكبر من أن يُرفع.
error-rate-limited = محاولات كثيرة. انتظر قليلًا ثم حاول مرة أخرى.
error-rate-limited-wait = محاولات كثيرة. حاول بعد { $wait }.
error-gpu-budget = نفد رصيد الأصوات التعبيرية لهذا اليوم. جرّب صوتًا قياسيًا.
error-gpu-budget-wait = نفد رصيد الأصوات التعبيرية لهذا اليوم. يتجدّد بعد { $wait }.
error-engine-waking = محرّك الأصوات يستيقظ الآن، وقد يستغرق ذلك دقيقة.
error-engine-failed = لم يتمكّن محرّك الأصوات من قراءة هذا الجزء.
error-upstream = تعذّر تحميل الصفحة. تحقّق من الرابط أو الصق النص بدلًا منه.
error-upgrade = يتوفّر إصدار جديد. جارٍ إعادة التحميل…
error-internal = حدث خطأ في الخادم. حاول مرة أخرى.

## Sign in
sign-in-lead = قائمة قراءتك، تُقرأ لك بصوت مسموع.
sign-in-title = سجّل الدخول إلى Streams Reader
sign-in-email-label = البريد الإلكتروني
sign-in-email-placeholder = you@example.com
sign-in-email-invalid = لا يبدو هذا عنوان بريد إلكتروني صحيحًا.
sign-in-continue = المتابعة بالبريد الإلكتروني
sign-in-or = أو
sign-in-with = المتابعة باستخدام { $provider }
sign-in-invite-label = رمز الدعوة
sign-in-invite-placeholder = مثل STREAMS-1234
sign-in-invite-hint = التسجيل في Streams Reader بالدعوة فقط حاليًا. اطلب الرمز ممّن دعاك.
sign-in-code-title = تحقّق من بريدك
sign-in-code-label = الرمز المكوّن من 6 أرقام
sign-in-verify = تسجيل الدخول
sign-in-link-hint = يمكنك أيضًا فتح الرابط الموجود في الرسالة على هذا الجهاز.
sign-in-resend = إرسال رمز جديد
sign-in-resend-wait = إعادة الإرسال بعد { $seconds } ث
sign-in-other-email = استخدام بريد آخر
sign-in-code-sent-before = أرسلنا رمزًا من 6 أرقام إلى{" "}
sign-in-code-sent-after = .
sign-in-new-here = جديد هنا؟ أدخل رمز الدعوة لإنشاء حسابك.
sign-in-have-invite = لديّ رمز دعوة
sign-in-oauth-invite = لإنشاء حساب بهذه الطريقة، أدخل بريدك الإلكتروني ورمز الدعوة أولًا.
sign-in-legal-before = بالمتابعة فإنك توافق على{" "}
sign-in-legal-and = {" "}و
sign-in-legal-after = .
legal-terms = الشروط
legal-privacy = الخصوصية
auth-error-offline = تعذّر الوصول إلى خدمة تسجيل الدخول. تحقّق من اتصالك.
auth-error-code = الرمز غير صحيح أو انتهت صلاحيته. حاول مرة أخرى أو اطلب رمزًا جديدًا.
auth-invite-needed = تحتاج إلى رمز دعوة لإنشاء حساب.
auth-error-invite = رمز الدعوة غير صالح أو سبق استخدامه.
auth-error-rate = محاولات كثيرة. انتظر دقيقة ثم حاول مجددًا.
auth-error-session = تم تسجيل خروجك. يُرجى تسجيل الدخول من جديد.
auth-error-other = لم ينجح تسجيل الدخول. حاول بعد قليل.
invite-used = رمز الدعوة هذا مُستخدَم من قبل.
invite-expired = انتهت صلاحية رمز الدعوة. اطلب رمزًا جديدًا.
invite-taken = رمز الدعوة هذا مرتبط ببريد إلكتروني آخر.
invite-unknown = لم نتعرّف على رمز الدعوة. تحقّق منه وحاول مجددًا.
auth-callback-busy = جارٍ تسجيل دخولك…
auth-callback-failed = لم يكتمل تسجيل الدخول
auth-callback-back = العودة إلى تسجيل الدخول

## First run
welcome-title = أهلًا بك في Streams Reader
welcome-lead = أمران سريعان ليعمل التشغيل بسلاسة، حتى والشاشة مطفأة.
welcome-install-title = أضِفه إلى الشاشة الرئيسية
welcome-install-line = يُفتح بملء الشاشة ويظهر في عناصر التحكم بالوسائط.
welcome-install = تثبيت
welcome-installed = تم التثبيت
welcome-installed-toast = تم التثبيت. افتحه من الشاشة الرئيسية.
welcome-install-manual = في Chrome، افتح القائمة ⋮ واختر «إضافة إلى الشاشة الرئيسية».
welcome-background-title = اسمح له بالعمل في الخلفية
welcome-background-line = قد يوقف Android الصوت بعد دقائق من إطفاء الشاشة. اسمح لـ Chrome بالعمل في الخلفية:
welcome-android-1 = افتح «الإعدادات» ثم «التطبيقات» ثم Chrome.
welcome-android-2 = اضغط على «استخدام البطارية للتطبيق».
welcome-android-3 = اختر «غير مقيّد».
welcome-samsung-line = في هواتف Samsung، أخرِج Chrome أيضًا من قوائم السكون:
welcome-samsung-1 = افتح «الإعدادات» ثم «البطارية» ثم «حدود الاستخدام في الخلفية».
welcome-samsung-2 = أزِل Chrome من «التطبيقات في وضع السكون» و«التطبيقات في وضع السكون العميق».
welcome-keys-title = استمع باستخدام لوحة المفاتيح
welcome-keys-line = في القارئ، مفتاح المسافة للتشغيل والإيقاف، وJ وL للتقديم والتأخير، و[ و] لتغيير السرعة.
welcome-start = ابدأ الاستماع

## Library
library-title = المكتبة
library-search = البحث في مكتبتك
library-search-placeholder = العنوان أو المؤلف أو الموقع
library-search-empty-line = جرّب كلمة أخرى، أو ابحث داخل مستند من القارئ.
library-continue = تابع الاستماع
library-all = كل المستندات
library-more = إجراءات أخرى
library-play = تشغيل
library-delete = حذف من المكتبة
library-removed = حُذف «{ $title }».
library-empty-title = لا شيء للاستماع إليه بعد
library-empty-line = أضِف مقالًا أو كتابًا أو ملاحظاتك الخاصة، ثم اضغط تشغيل.
library-share-tip = نصيحة: شارك أي صفحة من متصفحك مع Streams Reader.
library-unreachable = تعذّر الوصول إلى مكتبتك
library-unreachable-line = تحقّق من اتصالك، ثم حاول مرة أخرى.
library-preparing = جارٍ التحضير…
library-upload-failed = لم يكتمل الرفع
library-retry-gone = لم يعد هذا الملف على هذا الجهاز. احذفه ثم أضفه مرة أخرى.
doc-finished = انتهى
source-web = صفحة ويب
source-text = نص
source-book = كتاب
source-pdf = PDF
source-text-file = ملف نصي
source-markdown = Markdown

## Adding documents
add-title = إضافة إلى المكتبة
add-paste = لصق نص
add-paste-hint = مقال، فصل، أي شيء
add-link = لصق رابط
add-link-hint = المقالات والمدوّنات هي الأنسب
add-file = فتح ملف
add-file-hint = EPUB أو PDF أو TXT أو Markdown أو HTML
add-paste-title-label = العنوان (اختياري)
add-paste-text-label = النص
add-link-label = عنوان الويب
add-paste-placeholder = الصق النص أو اكتبه هنا.
add-paste-submit = إضافة النص
add-paste-empty = الصق بعض النص أولًا.
add-link-help = تُنظَّف الصفحة لتُقرأ المادة نفسها فقط.
add-link-submit = إضافة الرابط
add-link-invalid = لا يبدو هذا عنوان ويب صحيحًا.
add-file-error = تعذّر فتح الملف: { $detail }
add-share-tip = يمكنك أيضًا مشاركة صفحة أو ملف مع Streams Reader من تطبيقات أخرى.
add-done = أُضيف إلى مكتبتك.
import-stage-fetching = جارٍ جلب الصفحة…
import-stage-reading = جارٍ القراءة…
import-stage-uploading = جارٍ الإضافة إلى مكتبتك…
import-error-empty = لا يوجد فيه نص قابل للقراءة.
import-error-too-large = الحجم كبير جدًا ({ $detail }).
import-error-damaged = يبدو أن الملف تالف ولم تمكن قراءته.
import-error-unsupported = غير مدعوم: { $detail }.
import-error-pdf = تعذّرت قراءة ملف PDF ‏({ $detail }).
import-error-file-type = نوع الملف غير مدعوم. جرّب EPUB أو PDF أو TXT أو Markdown أو HTML.
import-error-worker = لم يبدأ المستورِد. أعد تحميل الصفحة وحاول مجددًا.

## Reader and player
reader-back = العودة إلى المكتبة
reader-more = المزيد
reader-play-here = شغّل من هنا
reader-back-to-voice = العودة إلى الصوت
reader-end = النهاية
reader-no-text = لا يحتوي هذا المستند على نص قابل للقراءة.
reader-hide-text = استماع دون نص
reader-hide-text-hint = يعرض الغلاف فقط أثناء التشغيل
reader-show-text = إظهار النص
reader-text-size = حجم النص
bookmark-add = ضع علامة هنا
bookmark-added = حُفظت العلامة.
bookmark-exists = هذا الموضع محفوظ في العلامات من قبل.
bookmark-remove = إزالة العلامة
reader-text-smaller = تصغير النص
reader-text-larger = تكبير النص
player-play = تشغيل
player-pause = إيقاف مؤقت
player-now-playing = قيد التشغيل
player-position = الموضع في المستند
player-chapter-left = متبقٍّ { $time } في الفصل
player-skip-back = رجوع { $seconds } ثانية
player-skip-forward = تقديم { $seconds } ثانية
player-error = توقّف التشغيل على نحو غير متوقع. اضغط تشغيل للمحاولة من جديد.
contents-title = المحتويات
search-doc-title = البحث في النص
search-doc-placeholder = ابحث عن كلمة أو عبارة
search-doc-hint = اكتب حرفين على الأقل.
search-doc-count = { $count ->
    [zero] لا نتائج
    [one] نتيجة واحدة
    [two] نتيجتان
    [few] { $count } نتائج
   *[other] { $count } نتيجة
}
search-no-results = لا نتائج
keys-title = اختصارات لوحة المفاتيح
keys-toggle = تشغيل أو إيقاف مؤقت
keys-skip = رجوع أو تقديم
keys-speed = أبطأ أو أسرع
keys-paragraph = الفقرة السابقة أو التالية
keys-follow = العودة إلى الصوت
keys-search = البحث في النص

## Speed and sleep
speed-title = سرعة التشغيل
speed-slower = أبطأ
speed-faster = أسرع
speed-hint = تبقى نبرة الصوت طبيعية بكل السرعات.
sleep-title = مؤقّت النوم
sleep-on = مؤقّت النوم مفعّل
sleep-off = إيقاف
sleep-minutes = { $minutes } د
sleep-end-chapter = نهاية الفصل
sleep-stops-in = يتوقّف بعد { $time }
sleep-hint = تخفت آخر عشر ثوانٍ بهدوء.
sleep-set = مؤقّت النوم: { $choice }
sleep-cleared = أُوقف مؤقّت النوم
sleep-ended = انتهى مؤقّت النوم. نومًا هانئًا.

## Voices
voices-title = الأصوات
voices-lead = اختر الصوت الذي يقرأ لك. اضغط على السمّاعة لتسمعه أولًا.
voices-favourites = المفضّلة
voices-recent = الأخيرة
voices-yours = أصواتك
voices-standard = قياسية
voices-standard-hint = استماع بلا حدود. يبدأ بسرعة.
voices-expressive = تعبيرية
voices-expressive-hint = تستهلك الرصيد اليومي للأصوات التعبيرية.
voices-expressive-line = أداء أغنى وأكثر عاطفة. كل معاينة وكل دقيقة صوت تستهلك من الرصيد اليومي.
voices-budget-today = رصيد اليوم التعبيري
voices-budget-numbers = { $left } من { $cap } ث · { $runs ->
        [zero] { $runs } مرة
        [one] مرة واحدة
        [two] مرتان
        [few] { $runs } مرات
        [many] { $runs } مرة
       *[other] { $runs } مرة
    }
voices-more = أصوات أكثر
voices-fewer = أصوات أقل
voice-title = الصوت
voice-switched = القراءة الآن بصوت { $name }
voice-default-set = أصبح { $name } صوتك الافتراضي
voice-budget-left = تبقّى اليوم نحو { $minutes } د من الصوت التعبيري و{ $runs ->
        [zero] { $runs } مرة
        [one] مرة واحدة
        [two] مرتان
        [few] { $runs } مرات
        [many] { $runs } مرة
       *[other] { $runs } مرة
    }.
voice-badge-expressive = تعبيري
voice-favourite = إضافة إلى المفضّلة
voice-unfavourite = إزالة من المفضّلة
voice-preview = معاينة { $name }
voice-manage = إدارة الصوت
voice-manage-line = الحذف يزيل الصوت والتسجيل الذي صُنع منه.
voice-delete = حذف هذا الصوت
voice-deleted = حُذف { $name }.
voice-ask-title = تشغيل معاينة تعبيرية؟
voice-ask-line = يُنشئ هذا عيّنة قصيرة على معالج الرسوميات ويستهلك نحو { $seconds } ثانية من رصيد اليوم.
voice-ask-left = تبقّى اليوم { $left ->
        [zero] { $left } ثانية
        [one] ثانية واحدة
        [two] ثانيتان
        [few] { $left } ثوانٍ
        [many] { $left } ثانية
       *[other] { $left } ثانية
    } و{ $runs ->
        [zero] { $runs } مرة
        [one] مرة واحدة
        [two] مرتان
        [few] { $runs } مرات
        [many] { $runs } مرة
       *[other] { $runs } مرة
    }.
voice-ask-play = تشغيل المعاينة
voice-state-needs-recording = يحتاج إلى تسجيل
voice-state-preparing = قيد التجهيز…
voice-state-failed = فشل
voice-yours-fallback = صوتك

accent-american = أمريكي
accent-british = بريطاني
gender-female = أنثى
gender-male = ذكر

## Recording a voice
record-title = سجّل صوتك
record-cta = سجّل صوتك
record-cta-line = اقرأ ثمانية أسطر قصيرة. نحو دقيقتين.
record-tips = اختر غرفة هادئة، وأمسك الهاتف على بُعد كفٍّ من فمك، واقرأ كل سطر إنجليزي بالإحساس المطلوب.
record-name = الاسم
record-default-name = صوتي
record-start = تسجيل
record-stop = إيقاف
record-again = إعادة التسجيل
record-progress = سُجّل { $done } من { $total } أسطر
record-create = إنشاء الصوت
record-uploading = جارٍ رفع تسجيلك…
record-preparing = جارٍ تجهيز صوتك…
record-wait = يستغرق ذلك دقيقة أو اثنتين. يمكنك مغادرة هذه الشاشة، فالعمل يستمر من دونك.
record-ready = صوتك جاهز
record-ready-line = يقرأ بالمحرّك التعبيري الذي يستهلك من الرصيد اليومي.
record-use = استخدم صوتي
record-failed-title = لم ينجح ذلك
record-back = العودة إلى التسجيل
record-line-good = السطر { $line } يبدو جيدًا.
record-failed = فشل التسجيل: { $detail }
record-every-line = سجّل كل الأسطر أولًا.
record-prepare-failed = تعذّر تحويل التسجيل إلى صوت. جرّب مرة أخرى في مكان أهدأ.
record-vanished = اختفى الصوت أثناء تجهيزه.
record-still-preparing = ما زال قيد التجهيز. سيظهر ضمن «أصواتك» عندما يجهز.
record-error-blocked = الوصول إلى الميكروفون محظور. اسمح به من إعدادات الموقع في المتصفح ثم حاول مجددًا.
record-error-no-mic = لم يُعثر على ميكروفون.
record-error-other = تعذّر بدء التسجيل: { $detail }
record-issue-clipping = الصوت مرتفع جدًا فتشوّه. ابتعد قليلًا واقرأ السطر مرة أخرى.
record-issue-noisy = هناك ضجيج في الخلفية. ابحث عن مكان أهدأ إن أمكن.
record-issue-quiet = الصوت منخفض جدًا. قرّب الهاتف قليلًا.
record-issue-short = كان قصيرًا جدًا. اقرأ السطر كاملًا.
mood-warm = بدفء
mood-calm = بهدوء
mood-delighted = بفرح
mood-sorry = باعتذار
mood-firm = بحزم
mood-curious = بفضول
mood-amused = بمرح
mood-gentle = بلطف

## Settings
settings-title = الإعدادات
settings-account = الحساب
settings-listening = الاستماع
settings-appearance = المظهر
settings-storage = التخزين
settings-engine = محرّك الأصوات
settings-about = حول التطبيق
settings-default-voice = الصوت الافتراضي
settings-default-speed = السرعة الافتراضية
settings-skip-back = مدة الرجوع
settings-skip-forward = مدة التقديم
settings-theme = السمة
settings-language = اللغة
theme-ink = حبر
theme-paper = ورق
theme-night = ليل
theme-system = النظام
language-auto = مثل لغة هذا الجهاز
language-auto-named = تلقائي ({ $language })
account-email = البريد الإلكتروني
account-offline = غير متصل
account-method-email = رمز البريد الإلكتروني
account-unlink = إلغاء الربط
account-unlinked = أُزيلت طريقة تسجيل الدخول.
account-methods = طرق تسجيل الدخول
account-linked = مرتبط
account-not-linked = غير مرتبط
account-link-short = ربط
account-sign-out = تسجيل الخروج
account-sign-out-all = تسجيل الخروج من كل الأجهزة
account-sign-out-all-confirm = تسجيل الخروج من كل الأجهزة؟
account-sign-out-all-line = سيحتاج كل هاتف وجهاز لوحي وحاسوب مسجَّل الدخول بهذا الحساب إلى تسجيل الدخول من جديد.
account-sign-out-all-action = الخروج من كل الأجهزة
storage-used = المستخدَم على هذا الجهاز
storage-clear = مسح الصوت المحفوظ
storage-hint = يُحفظ الصوت الذي استمعت إليه لتعيد تشغيله دون اتصال.
storage-clear-title = مسح الصوت المحفوظ؟
storage-clear-line = يوفّر المسح مساحة، وتُنزَّل المقاطع من جديد عند الحاجة.
storage-clear-confirm = مسح
storage-cleared = مُسح الصوت المحفوظ.
engine-voice = الحالة
engine-awake = يعمل
engine-asleep = نائم (يستيقظ عند أول استخدام)
engine-starting = قيد التشغيل
engine-unavailable = غير متاح
engine-gpu-today = الرصيد التعبيري اليوم
engine-gpu-left = { $left } من { $cap } ث · { $runs ->
        [zero] { $runs } مرة
        [one] مرة واحدة
        [two] مرتان
        [few] { $runs } مرات
        [many] { $runs } مرة
       *[other] { $runs } مرة
    }
engine-queue = بانتظار الإنشاء
engine-refresh = تحديث الحالة
about-version = الإصدار
about-setup = التثبيت والتشغيل في الخلفية
about-source = الشفرة المصدرية
about-colophon = Streams Reader · مجاني ومفتوح المصدر

## Not found
not-found-title = لا شيء هنا
not-found-line = هذه الصفحة غير موجودة.
not-found-back = العودة إلى المكتبة
