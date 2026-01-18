# Roadmap

# Bugfixes

- Add support for EPUB 3 TOCs, likely via an upstream PR to the `epub` crate.
- Add support for SVG spine items.

# Potential-bugfix exploration

- Ensure UTF-16-encoded XHTML/SVG gets reencoded to UTF-8 and has all metadata specifying it as UTF-16 also updated to specify it as UTF-8 instead. <meta> elements in the head, and such.
- Any way to work on Linux if `xdg-open` isn't installed, or to detect its non-installation and print a warning, or to legibly depend on it, or such?

# Release

- Look into crates.io's "Trusted Publishing" feature as an alternative to having `cargo publish` as a manual step in the release process
- Build msi installer for integration with Winget
- Add to whatever package managers are convenient to add to, including Winget for Windows, Homebrew for Mac, and the AUR for Linux.

# Features

- Add more graceful error-handling for failure to open one book in a specified bunch, so the error doesn't prevent other books from being opened
- Add test to ensure that print-output from `cargo run` and print-output from `cargo run -- help` are identical
- Explore options for alternate format support:
    - mobi: `mobi` crate exists; see if it's any good, and see if the mobi format has good native browser support
    - azw3: no obviously-relevant crate as of search on 2026-01-02; might need to rely on external tools here
    - cb[z/r/others?]: Might be easy to fit into the reader/navigation framework? But also might be a bad fit better-suited to more specialized local browsers. Plausibly worth experimentation.
    - pdf: the hard one. Probably don't. But at least consider it. See what pdf.js is capable of, maybe?
- Allow index to display TOC mapped to spine in the event that it's linear-relative-to-TOC *except for the nonlinear TOC-items*, even if the nonlinear TOC-items are out-of-order, as long as each *individual* nonlinear TOC-item shows up only in one block of TOC
- Maybe make library path configurable, in case people want to store their libraries somewhere other than the default ephemeral data dir?
- Add `library open` command to open library books by ID
- Add more `config` subcommands. Including `config set [parameter] [value]` for git-style command-line config-setup, and maybe `config edit` or something to that effect to open the config file directly in an editor?
- In the index XHTML, detect language from book and set `html` element's `lang` attribute to something other than `en` if applicable. Same in injected navigation pages.
- Get an icon to make it integrate nicely into desktop environments for people who don't want to invoke it only via CLI
- Maybe code up some sort of GUIish manager for it for people who want to do library-management and/or configuration via GUI rather than CLI? Pretty late in the game if so, though.
- Maybe move TOC to display above the spine-and-TOC pair, in the index, to improve ergonomics for users uninterested in the guts?
- Maybe render nonlinear spine items at reduced opacity, as a visual indicator of their nonlinearity?
- Implement styles with light/dark branching
- Save book last-opened style in library file even when the cached book-copy is deleted, so that users with small caches don't lose their styles between runs.
- Experiment and see if the link color in the black theme should be made lighter
- Look more into alternatives to `cli-table` for print outputs; possibly just roll my own somehow, even
