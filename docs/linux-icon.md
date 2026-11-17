# Linux application icon

The Gravity mark is embedded in the binary for the X11 window/taskbar icon.
The Linux window identifies itself with desktop ID `dev.lapce.lapce`, including
Wayland app_id. The bundled desktop entry and theme icon use that same ID.

For an extracted Linux build, keep all files together and run
`./install-desktop.sh` once. This registers the current bundle for your user
under `~/.local/share`, without root. Re-run after moving the bundle. A custom
absolute installation prefix can be supplied for packaging or testing.

The installer includes PNG and scalable SVG icons. No AppImage or deb builder
is currently present in these workflows; both workflow and tagged-release tar
bundles include the same installer and artwork.
