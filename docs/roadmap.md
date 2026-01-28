# Roadmap

# Bugfixes

- Add support for EPUB 3 TOCs, likely via an upstream PR to the `epub` crate.
- Add support for SVG spine items.
- Update `epub` crate to include the NCX-path-fix once there's a release with that
- Fix handling of `<br>` tags, which currently are (to browser parsers' eyes) doubled
	- This is because they're [void elements](https://developer.mozilla.org/en-US/docs/Glossary/Void_element). Probably the answer here is a generalized policy of, when writing void elements, writing them in self-closing form rather than non-self-closing form. Figure out how to make the XML writer vary on that axis.
- Update back away from `src` and towards a pairing of `srcdoc` plus a compensatory `base` tag, so that CORS doesn't interfere with fragment-following

# Potential-bugfix exploration

- Ensure UTF-16-encoded XHTML/SVG gets reencoded to UTF-8 and has all metadata specifying it as UTF-16 also updated to specify it as UTF-8 instead. `<meta>` elements in the head, and such.
- Any way to work on Linux if `xdg-open` isn't installed, or to detect its non-installation and print a warning, or to legibly depend on it, or such?

# Release

- Look into crates.io's "Trusted Publishing" feature as an alternative to having `cargo publish` as a manual step in the release process
- Build msi installer for integration with Winget, then submit to Winget
- Once I've done basic testing on Mac, set up Homebrew packaging
- Add to whatever other package managers are convenient to add to. Linux AUR? Details TBD.

# Features

- Add more graceful error-handling for failure to open one book in a specified bunch, so the error doesn't prevent other books from being opened
- Allow index to display TOC mapped to spine in the event that it's linear-relative-to-TOC *except for the nonlinear TOC-items*, even if the nonlinear TOC-items are out-of-order, as long as each *individual* nonlinear TOC-item shows up only in one block of TOC
- Add `library open` command to open library books by ID
- Add more `config` subcommands. Including `config set [parameter] [value]` for git-style command-line config-setup, and maybe `config edit` or something to that effect to open the config file directly in an editor?
- In the index XHTML, detect language from book and set `html` element's `lang` attribute to something other than `en` if applicable. Same in injected navigation pages.
- Get an icon to help integrate nicely into desktop environments for people who don't want to invoke it only via CLI
- Implement styles with light/dark branching
- Save book last-opened style in library file even when the cached book-copy is deleted, so that users with small caches don't lose their styles between runs.
- Add option to dump only, without opening in browser
- Add generally-more-robust unit-testing, where practical
- Add styles: font-family, font-size, opened-link-color, custom CSS
- Add flag to open to a specific spine-section, probably 1-indexed
- Render nonlinear spine items at reduced opacity in the index, as a visual indicator of their nonlinearity
- Focus the inside of the section iframe on load, so that scrolling via page-down works without needing the frame clicked into first
- Make library path configurable, in case people want to store their libraries somewhere other than the default ephemeral data dir or the default ephemeral data dir is non-UTF-8
- Explicitly remove script tags from book sections, until such a time as I figure out safe sandboxing.

# Feature Consideration/Research

- Explore options for alternate format support:
	- mobi: `mobi` crate exists; see if it's any good, and see if the mobi format has good native browser support
	- azw3: no obviously-relevant crate as of search on 2026-01-02; might need to rely on external tools here
	- cb[z/r/others?]: Might be easy to fit into the reader/navigation framework? But also might be a bad fit better-suited to more specialized local browsers. Plausibly worth experimentation.
	- pdf: the hard one. Probably don't. But at least consider it. See what pdf.js is capable of, maybe?
- Maybe alternately allow reading config from next to executable, in case the system-specific config-location is broken?
- Maybe code up some sort of GUIish manager for people who want to do library-management and/or configuration via GUI rather than CLI? Pretty late in the game if so, though.
- Maybe move TOC to display above the spine-and-TOC pair, in the index, to improve ergonomics for users uninterested in the guts?
- Experiment and see if the link color in the black theme should be made lighter
- Look more into alternatives to `cli-table` for print outputs; possibly just roll my own, even
- Consider renaming the CLI `stylesheets` parameter to `stylesheet`, since users will rarely want to use more than one, it might get confusing, and clap's `long = "stylesheet"` option is broken.
	- Also submit an issue report with clap about that brokenness
- Consider whether to add styles: line spacing, paragraph spacing, start-of-paragraph indentation
- List word count per spine-section, in the spine-section-listing?
	- In theory maybe even implement an optional progress bar based on this, via some sort of clever hardcoding? If practical to implement in CSS-without-JS.
- Decide whether it's worth the JS-costs and the reduction-in-standard-browser-behavior costs to add keyboard navigation
- Research `thiserror` as an alternative to `anyhow`, with particular interest in whether it can let me add more semantic details to my error-type (error-enum of Unreachable, InternalError, IntendedCrash, PrintWarning?) without sacrificing too much of `anyhow`'s ergonomics
- Make the navigation UI nicer.
	- Next/previous buttons in margins, rather than in a bottom-popup?
	- Switch bottom-popup from its current floating design to a "displaces the iframe a bit if the user mouses over the frame's bottom or top" design?
	- Consider if it's practical to add a dropdown navigation-menu in the style of FFN / AO3, and if so how to do it.
	- Other?
- Consider defining the rest of the ProjectDirs fields, not just the application name
- Reasily-style page-number-display?
- Factor out `pathdiff` in favor of diffing `file://` URLs?
- Traitify XML helpers as with path helpers?
