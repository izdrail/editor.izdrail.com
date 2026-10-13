<h1 align="center">
  <img src="extra/images/logo.png" width=200 height=200/><br>
  editor.izdrail.com
</h1>

<h4 align="center">Lightning-fast And Powerful Code Editor</h4>

<div align="center">
  <a href="https://github.com/izdrail/editor.izdrail.com/actions/workflows/linux-build.yml" target="_blank">
    <img src="https://github.com/izdrail/editor.izdrail.com/actions/workflows/linux-build.yml/badge.svg" />
  </a>
  <a href="https://discord.gg/n8tGJ6Rn6D" target="_blank">
    <img src="https://img.shields.io/discord/946858761413328946?logo=discord" />
  </a>
  <a href="https://docs.lapce.dev" target="_blank">
      <img src="https://img.shields.io/static/v1?label=Docs&message=docs.lapce.dev&color=blue" alt="Lapce Docs">
  </a>
</div>
<br/>


This editor is built on [Lapce](https://github.com/lapce/lapce), an open source code editor written in Rust, copyright the Lapce contributors and released under the Apache License 2.0. This repository carries the full Lapce source with the following changes: the language server (LSP) and WASI plugin services are removed, syntax highlighting uses Lexilla (`lexilla-rs/`), a keymap preset system is added (compact-65 default, vscode, phpstorm, notepad-plus-plus, lapce legacy), local code folding groundwork is added, and GitHub workflows build the Linux release binary.

Lapce (IPA: /læps/) is written in pure Rust, with a UI in [Floem](https://github.com/lapce/floem). It is designed with [Rope Science](https://xi-editor.io/docs/rope_science_00.html) from the [Xi-Editor](https://github.com/xi-editor/xi-editor), enabling lightning-fast computation, and leverages [wgpu](https://github.com/gfx-rs/wgpu) for rendering. More information about the features of Lapce can be found on the [main website](https://lapce.dev) and user documentation can be found on [GitBook](https://docs.lapce.dev/).

![](https://github.com/lapce/lapce/blob/master/extra/images/screenshot.png?raw=true)

## Features

* Built-in LSP ([Language Server Protocol](https://microsoft.github.io/language-server-protocol/)) support to give you intelligent code features such as: completion, diagnostics and code actions
* Modal editing support as first class citizen (Vim-like, and toggleable)
* Built-in remote development support inspired by [VSCode Remote Development](https://code.visualstudio.com/docs/remote/remote-overview). Enjoy the benefits of a "local" experience, and seamlessly gain the full power of a remote system. We also have [Lapdev](https://lap.dev/) which can help manage your remote dev environments. 
* Plugins can be written in programming languages that can compile to the [WASI](https://wasi.dev/) format (C, Rust, [AssemblyScript](https://www.assemblyscript.org/))
* Built-in terminal, so you can execute commands in your workspace, without leaving Lapce.

## Installation

Linux builds are attached to this repository's [releases](https://github.com/izdrail/editor.izdrail.com/releases), and every push publishes a Linux binary as a workflow artifact. The package manager guide in [`docs/installing-with-package-manager.md`](docs/installing-with-package-manager.md) describes upstream Lapce packages.
If you'd like to compile from source, you can find the [guide](docs/building-from-source.md).

## Contributing

Guidelines for contributing to Lapce can be found in [`CONTRIBUTING.md`](CONTRIBUTING.md).

## Feedback & Contact

The most popular place for Lapce developers and users is on the [Discord server](https://discord.gg/n8tGJ6Rn6D).

Or, join the discussion on [Reddit](https://www.reddit.com/r/lapce/) where we are just getting started.

There is also a [Matrix Space](https://matrix.to/#/#lapce-editor:matrix.org), which is linked to the content from the Discord server.

## License

This project is a derivative of Lapce, copyright the Lapce contributors, and is released under the Apache License Version 2.0, the same license as upstream. Upstream copyright and attribution notices are retained. You may contribute to this project, or use the code as you please as long as you adhere to its conditions. You can find a copy of the license text here: [`LICENSE`](LICENSE).
