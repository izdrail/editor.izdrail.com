# Hosted Linux acceptance

Pushes to main run the lexer binding tests, focused conversation tests and full Rust workspace suite on GitHub-hosted Ubuntu 22.04. The release build starts only after those pass. Manual runs are restricted to main by job guards. Version tags do not start builds or publish releases.

Test and release jobs have separate caches and disks. Tests disable debug symbols and incremental compilation, use two build jobs and two test threads, and allow 90 minutes. Each job requires at least 15 GiB free and stops without deleting data if the floor is not met. Cold caches may take longer than warm builds. The test logs are retained for 14 days; release downloads for 30 days.

The release artifact includes Gravity and its proxy, with version checks and SHA-256 checksums. It is a downloadable build, not a deployment. No host access, self-hosted runner or tag is needed. GUI acceptance must still be inspected separately before a UI change is called accepted.

The hosted release job also launches the built editor under Xvfb with a fresh synthetic project and local-only settings. It captures the initial panel, draft, mode/model controls and lists, Send focus without activation, focus return and outside focus. Screenshots and logs are downloadable as composer-gui-acceptance. A successful capture is not visual acceptance: the images must be opened and inspected.

The capture flow also queries for a file absent from the cached index, creates that synthetic file, presses F5 and captures the refreshed result and Tab insertion. Review all three images to verify discovery of new files, not only a refresh of existing results. Both synthetic source files must remain unchanged; Send is never activated.

The GUI launch explicitly requests a fresh window and preserves profile startup logs, including early exits. A process exit before the first capture fails acceptance and does not count as visual verification.

After proving new-file insertion, the synthetic file is removed and the same query refreshed again. Review the no-match and subsequent Tab images to verify the deleted cached suggestion is not inserted and Tab safely moves to mode.

Ten synthetic similarly named files exercise the eight-row suggestion cap. Captures show PageDown selecting row six, End selecting row eight, Home returning to row one and Tab inserting that row. The footer must count offered rows, not all ten matching files.
