# Release Workflow

## Release Dependencies

- Cargo
	- `cargo-msrv`
- Dist

## Before release

### Update MSRV

Comment out the `rust-version` field from `Cargo.toml`, then run:

```
cargo msrv find
```

Uncomment the field and set it to match the command's output MSRV.

### Update version

Edit `Cargo.toml`'s `version` key to the version number being released, incrementing major or minor or patch version as befits the changes since last release.

### Update changelog

Edit `CHANGELOG.md` to list changes associated with the new version.

### Ensure all tests pass

```
cargo test
```

### Ensure Cargo package builds

```
cargo package --allow-dirty
```

After building the package, browse `target/package/rib-{version}` to make sure the package contains only relevant-and-desired files.

### Ensure that Dist's plan is as expected

```
dist plan
```

As of `rib` version 0.1.0, under `dist` 0.30.3, the output plan looks as follows:

```
source.tar.gz
	[checksum] source.tar.gz.sha256
sha256.sum
rib-aarch64-apple-darwin.tar.xz
	[bin] rib
	[misc] LICENSE.md, README.md
	[checksum] rib-aarch64-apple-darwin.tar.xz.sha256
rib-x86_64-pc-windows-msvc.zip
	[bin] rib.exe
	[misc] LICENSE.md, README.md
	[checksum] rib-x86_64-pc-windows-msvc.zip.sha256
rib-x86_64-unknown-linux-gnu.tar.xz
	[bin] rib
	[misc] LICENSE.md, README.md
	[checksum] rib-x86_64-unknown-linux-gnu.tar.xz.sha256
```

## Release

### Commit and push

Create and push a Git commit in the usual manner.

### Create and push git tag

```
git tag v{VERSION}
git push --tags
```

VERSION should be set to the version defined in `Cargo.toml` at the start of pre-release testing.

### Publish to crates.io

```
cargo publish
```
