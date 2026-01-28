# Rib

[![crates.io](https://img.shields.io/crates/v/rib.svg)](https://crates.io/crates/rib)
[![sponsors](https://img.shields.io/github/sponsors/twilight-flower)](https://github.com/sponsors/twilight-flower)

Read EPUBs in your web browser!

A lightweight desktop EPUB reader. Name is short for '**R**ead **i**n **b**rowser'. Currently in early development.

Tested on Windows and Linux; currently untested on Mac, but theoretically should work there as well.

## Usage

Open `book.epub` in your default web browser with default style settings:

```
rib book.epub
```

View help menu for advanced functionality:

```
rib help
```

## Installation

Rib has a dependency on `xdg-open` on Linux. It has no known external dependencies on Windows or Mac.

### Option 1: Install via Cargo

```
cargo install --locked rib
```

`cargo-binstall` is also supported.

### Option 2: Download via GitHub Releases

See [the releases page](https://github.com/twilight-flower/rib/releases) for download links. For convenience, you'll likely want to extract the release's executable to somewhere on your PATH.

### Option 3: Build Manually

The source tarball from the latest release can be downloaded [here](http://github.com/twilight-flower/rib/releases/latest/download/source.tar.gz), or (along with older versions' source tarballs) from [the releases page](https://github.com/twilight-flower/rib/releases). After extracting the source, build via:

```
cargo build --release
```

The executable will be built in the `target/release` subdirectory. For convenience, you'll likely want to move it to somewhere on your PATH.
