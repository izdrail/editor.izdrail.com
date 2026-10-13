# Updates

The editor has no self-update. The upstream Lapce updater (update checks against
github.com/lapce/lapce, the "Restart to update" menu entry and badge, the
`restart_to_update` command and the download/extract/restart code) was removed
because it would have replaced this fork with upstream Lapce binaries. The
`updater` cargo feature is gone too, and nothing contacts a release server.

Install a new build by replacing the binary. A future update channel for this
fork would belong in a new module wired in at app start-up and in the title bar
settings menu, where the old entry sat.
