# Run Test at Cursor / Run Test File (PHPUnit and Pest)

Command palette: "Run Test at Cursor" and "Run Test File" run the active PHP test in the terminal.
No keybinding is assigned; add one in `keymaps.toml` for `run_test_at_cursor` / `run_test_file`.

- Test files: a `.php` file under a `tests/` (or `Tests/`) folder, or named `*Test.php`.
- Runner: `php artisan test` when `artisan` exists (works for PHPUnit and Pest), else `vendor/bin/pest`,
  else `vendor/bin/phpunit`, run from the workspace root. Nothing is run unless you pick the command.
- Test at cursor: the enclosing PHPUnit method named `test*` or marked `#[Test]` / `@test`
  (`--filter testName`, which also matches names that start with the same text and data-set variants),
  or the enclosing top-level Pest `it('...')` / `test('...')` (`--filter` with its description; regex
  characters in it match any character). Between tests, in a helper method, or at class level, the
  whole file runs instead.
- Save first: the runner reads the file from disk, while the test under the cursor is found in the
  open buffer. Buffers over 1 MB run as a whole file.
- Not a test file, no runner, or no folder open: a warning toast says so.
- The run appears under Run and Debug like other commands (re-run from the palette list). Debugging a
  test with Xdebug is not wired.
