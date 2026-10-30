# Auth email wording, English (the fallback for every other language).
# Variables: $brand (product name), $site (site address), $code (one-time code), $minutes (code lifetime),
# $new_email (the address being confirmed). Keep every message on one line.

greeting = Hello,
code-label = Your code
code-or-link = Or open the button on this device.
button-fallback = If the button does not work, copy this address into your browser:
ignore = If you did not ask for this, you can ignore this email. Nobody can sign in without the code.
footer = Sent by { $brand } · { $site }

signup-subject = { $code } is your { $brand } code
signup-title = Finish creating your account
signup-body = Welcome to { $brand }. Enter this code in the app to finish creating your account. It works once and expires in { $minutes } minutes.
signup-button = Confirm and sign in

magiclink-subject = { $code } is your { $brand } sign-in code
magiclink-title = Your sign-in code
magiclink-body = Enter this code to sign in. It works once and expires in { $minutes } minutes.
magiclink-button = Sign in

recovery-subject = Get back into your { $brand } account
recovery-title = Get back into your account
recovery-body = Enter this code or open the button to sign in again. It works once and expires in { $minutes } minutes.
recovery-button = Sign in

email-change-subject = Confirm your new { $brand } email address
email-change-title = Confirm the email change
email-change-body = You asked to change the email address of your { $brand } account to { $new_email }. Enter this code or open the button to confirm.
email-change-button = Confirm the change

invite-subject = You are invited to { $brand }
invite-title = You are invited
invite-body = Someone invited you to { $brand }, an app that reads your articles and books aloud. Accept the invitation to create your account.
invite-button = Accept the invitation

reauthentication-subject = { $code } is your { $brand } confirmation code
reauthentication-title = Confirm it is you
reauthentication-body = Enter this code to confirm that it is you. It works once and expires in { $minutes } minutes.
