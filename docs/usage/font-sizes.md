# Font sizes

The default editor and UI fonts are 12px. The terminal inherits the editor font size unless its own size is set. Existing explicit font-size settings keep their values.

Open Settings and change **Editor: Font Size** for source code and inherited terminal text, or **UI: Font Size** for the sidebar, tabs and other interface text. You can also edit these values in your user settings:

```toml
[editor]
font-size = 12

[ui]
font-size = 12

[terminal]
font-size = 0 # inherit editor size
```

Choose larger values if they are more comfortable. The smaller defaults do not overwrite your settings or change system display scaling.
