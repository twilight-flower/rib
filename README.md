# Rib

Read EPUBs in your web browser!

A lightweight desktop EPUB reader. Name is short for '**R**ead **i**n **b**rowser'. Currently in early development.

Tested on Windows and Linux; currently untested on Mac, but theoretically should work there as well.

## Installation

For the moment, see the Development section below and build it yourself. More install options upcoming.

Has a dependency on `xdg-open` on Linux. No known external dependencies on Windows or Mac.

## Usage

Open book with default settings:

```
rib book.epub
```

View help menu for advanced functionality:

```
rib help
```

## Development

Build dependencies:

- Cargo

Build:

```
cargo build --release
```
