# Rib

Read EPUBs in your web browser!

A lightweight desktop EPUB reader. Name is short for '**R**ead **i**n **b**rowser'. Currently in early development.

Tested on Windows and Linux; currently untested on Mac, but theoretically should work there as well.

## Usage

Open book in your default web browser with default style settings:

```
rib book.epub
```

View help menu for advanced functionality:

```
rib help
```

## Installation

Rib has a dependency on `xdg-open` on Linux. It has no known external dependencies on Windows or Mac.

### Install via Cargo

```
cargo install --locked rib
```

### Download via GitHub releases

See [the releases page](https://github.com/twilight-flower/rib/releases) for download links. For convenience, you'll likely want to extract the release's executable to somewhere on your PATH.

### Build manually

```
git clone git@github.com:twilight-flower/rib.git
cd rib
cargo build --release
```

The output executable will appear in the `target/release` subdirectory. For convenience, you'll likely want to move it to somewhere on your PATH.
