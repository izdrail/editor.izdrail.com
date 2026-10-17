# Channel / disk hover

Hovering the name inside one of Laravel's named-driver calls shows the
entry's driver from `config/*.php`, with the file and line - no language
server round trip:

    Storage::disk('s3')   ->  **`s3`** - filesystem disk, driver `s3`
    Log::channel('daily') ->  **`daily`** - log channel, driver `daily`
    Cache::store('redis') ->  **`redis`** - cache store, driver `redis`
    Mail::mailer('ses')   ->  **`ses`** - mailer, driver `ses`
    Auth::guard('api')    ->  **`api`** - auth guard, driver `...`
    DB::connection('mysql') ->  **`mysql`** - database connection, driver `mysql`
    Queue::connection('redis') / Redis::connection(...) -> connection driver

Also covers the `->disk(...)`, `->channel(...)` etc. builder forms, matching
the go-to-definition targets (Ctrl+Click jumps to the entry; hover explains
it in place).

How it works:

- Pure text resolution, same as the go-to: the quoted name under the cursor
  is looked up in `config/filesystems.php`, `config/logging.php`,
  `config/cache.php`, `config/mail.php`, `config/auth.php`,
  `config/database.php` and `config/queue.php`.
- The entry may be a single line or the head of a nested array; a small
  window of following lines is scanned for the `'driver'` key (`'transport'`
  for mail mailers).
- Nothing is executed; project PHP files are read as text (capped at 1 MB).

Limits: the first matching entry in the file wins, matching the go-to
behaviour; entries built with `Config::get()` or dynamic keys are not
tracked.
