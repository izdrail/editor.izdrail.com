# Go to Migration

"Go to Migration" in the command palette lists the files in `database/migrations`, newest first, and opens the one you pick.

`migration  2024_05_05_000000_add_flag  (table users)`: the name plus up to 8 distinct `Schema::create|table|dropIfExists|drop|rename|dropColumns('name')` calls found in the file (the `down()` ones too). It opens at the first such call, or the top of the file when there is none.

Newest first is by file name, since Laravel names start with a timestamp. Migrations whose names do not (squashed schema, custom names) sort by name.

Limits: static text scan, nothing is run and the database is not asked, so applied or pending state is not shown. Table names built from variables are skipped. Depth 2 (one subfolder), 3000 files, 1 MB per file, 5000 entries, symlinks skipped.
