# Hosted Linux acceptance

Pushes to main run the lexer binding tests, focused conversation tests and full Rust workspace suite on GitHub-hosted Ubuntu 22.04. The release build starts only after those pass. Manual runs are restricted to main by job guards. Version tags do not start builds or publish releases.

Test and release jobs have separate caches and disks. Tests disable debug symbols and incremental compilation, use two build jobs and two test threads, and allow 90 minutes. Each job requires at least 15 GiB free and stops without deleting data if the floor is not met. Cold caches may take longer than warm builds. The test logs are retained for 14 days; release downloads for 30 days.

The release artifact includes Gravity and its proxy, with version checks and SHA-256 checksums. It is a downloadable build, not a deployment. No host access, self-hosted runner or tag is needed. GUI acceptance must still be inspected separately before a UI change is called accepted.

The hosted release job also launches the built editor under Xvfb with a fresh synthetic project and local-only settings. It captures the initial panel, draft, mode/model controls and lists, Send focus without activation, focus return and outside focus. Screenshots and logs are downloadable as composer-gui-acceptance. A successful capture is not visual acceptance: the images must be opened and inspected.
