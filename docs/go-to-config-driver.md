# Go to Laravel driver config (disk, channel, store, mailer, guard, connection)

In a PHP file, put the cursor on the name in one of these calls and run Go to Definition:

| Call | Opens |
| --- | --- |
| `Storage::disk('s3')`, `->disk('s3')` | `config/filesystems.php` |
| `Log::channel('slack')` | `config/logging.php` |
| `Cache::store('redis')` | `config/cache.php` |
| `Mail::mailer('ses')` | `config/mail.php` |
| `Auth::guard('api')`, `Auth::shouldUse('web')` | `config/auth.php` |
| `DB::connection('mysql')`, `Redis::connection('cache')` | `config/database.php` |
| `Queue::connection('sqs')` | `config/queue.php` |

It lands on the first `'name' => ...` line in that file, so a name reused in one config file can open
an earlier entry. Not found: config published under another path, names built at runtime. Limit:
1 MB per file. Nothing is executed. If nothing is found, normal Go to Definition runs.
