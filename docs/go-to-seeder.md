# Go to seeder

"Go to Definition" on a `PostSeeder::class` constant in a PHP file - typically
`$this->call(PostSeeder::class)` or `$this->call([...])` in `DatabaseSeeder` - opens
`database/seeders/PostSeeder.php` at the `class PostSeeder` line. The legacy
`database/seeds/` folder is used when `database/seeders/` has no such file. A fully
qualified `\Database\Seeders\PostSeeder::class` resolves the same way.

Only the conventional file name is tried; seeders in custom folders are not followed.
The name must end in `Seeder`, be capitalised and sit right before `::class`, so
`PostSeeder::create()` and `new PostSeeder` are left alone. Static text scan only; no
PHP is run. Files over 1 MB are not read.

Status: unit tests cover the call shapes, the legacy fallback, name boundaries
(`PostSeederBase` does not match), missing files, the 1 MB cap and non-ASCII fuzz.
The editor jump is not yet verified in the GUI.
