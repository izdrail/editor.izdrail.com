# Mailer completion (item 401)

`Mail::mailer('sm` and `Mail::driver('sm` offer the mailers the
project declares, from the `'mailers' => [...]` array of
`config/mail.php` (Laravel 12.x `MailManager`: `mailer($name =
null)`, `driver($driver = null)` as its alias).

Only a scalar name completes; an array argument stays silent. Only
the `mailers` array's top-level keys are harvested: the `'default'`
key, per-mailer settings one level deeper and commented-out mailers
invent nothing, comments are masked first, and a file containing
heredoc/nowdoc is skipped whole (the item 390d guard). The receiver
must be exactly `Mail::` - `MyMail::mailer(...)` and
`$mail->mailer(...)` stay silent - and a call written inside a
comment or a string literal is not a call.

Tests: `detects_mailer_context`, `offers_mail_mailers`.
