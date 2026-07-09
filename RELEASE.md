# Release Process

This document describes how to release JEdit and how users install it on
Windows, macOS, and Linux.

## Goals

* Use Git as the source of truth for released versions.
* Publish GitHub Releases with direct download artifacts for Windows, macOS,
  and Linux.
* Avoid publishing this fork to package managers such as WinGet, Homebrew,
  Snapcraft, or distro repositories.
* Preserve a source-build installer for Unix-like systems.

## Versioning

JEdit uses SemVer-style versions and `vX.Y.Z` Git tags. This fork tracks the
upstream minor and patch version under major version 20: upstream `v2.0.0`
corresponds to JEdit `v20.0.0`, upstream `v2.1.3` corresponds to JEdit
`v20.1.3`, and so on.

Update these files together for every release:

* `crates/edit/Cargo.toml`
* `Cargo.lock`
* `assets/snapcraft.yaml`, if Snap metadata is kept in the tree

Use patch releases for bug fixes and minor releases for user-visible features.
Reserve the next major version for incompatible changes to behavior,
configuration, or supported platforms.

## Pre-Release Checklist

Start from a clean checkout of `main`.

```sh
git switch main
git pull --ff-only
cargo fmt --all -- --check
cargo test --all-features --all-targets
cargo clippy --all-features --all-targets -- --no-deps --deny warnings
```

For changes that touch ICU lookup or package-maintainer settings, also run:

```sh
cargo test -- --ignored
```

## Prepare the Version Commit

Create a release branch, update the version, and open a pull request.

```sh
git switch -c release/vX.Y.Z
```

After editing the version files, refresh the lockfile:

```sh
cargo check -p edit
```

Commit the version update:

```sh
git add crates/edit/Cargo.toml Cargo.lock assets/snapcraft.yaml
git commit -m "Release vX.Y.Z"
```

Merge the pull request after CI passes.

## Tag the Release

After the release commit is on `main`, create and push an annotated tag:

```sh
git switch main
git pull --ff-only
git tag -a vX.Y.Z -m "JEdit vX.Y.Z"
git push origin vX.Y.Z
```

Tags should not be moved after publication. If a release has a problem, publish
a new patch release.

## Build Release Artifacts

Windows release artifacts are produced by `.pipelines/release.yml`.

Run the pipeline for the `vX.Y.Z` tag with:

* `official`: `Official`
* `createvpack`: `true`
* `buildPlatforms`: `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`

The expected Windows GitHub Release assets are:

* `jedit-X.Y.Z-x86_64-windows.zip`
* `jedit-X.Y.Z-aarch64-windows.zip`

Each archive should contain `jedit.exe` and its matching debug symbols. The
pipeline signs `jedit.exe` before packaging.

macOS and Linux release artifacts are produced by
`.github/workflows/release-unix.yml` when a GitHub Release is published. The
same workflow can also be run manually for a tag.

The expected Unix GitHub Release assets are:

* `jedit-X.Y.Z-x86_64-linux.tar.gz`
* `jedit-X.Y.Z-x86_64-macos.tar.gz`
* `jedit-X.Y.Z-aarch64-macos.tar.gz`

Each archive contains the `jedit` binary, `LICENSE`, `README.md`, and the
`jedit.1` manpage. The source-build installer remains available as a fallback
and builds from the tag published on GitHub.

## Publish the GitHub Release

Create a GitHub Release from the pushed tag.

1. Use the tag name as the release title, for example `vX.Y.Z`.
2. Attach the Windows release archives.
3. Include a short changelog with user-visible changes and fixes.
4. Mention that this fork is installed directly from GitHub Releases or the
   source-build installer.
5. Mark the release as a pre-release only for preview builds.
6. Publish the release.
7. Confirm the macOS and Linux release workflow uploads its archives.

## Install Channels

### Windows

Primary user install path:

```powershell
# Download and extract jedit-X.Y.Z-x86_64-windows.zip, then run:
jedit.exe
```

Release owner responsibility:

* Publish the GitHub Release with both Windows zip assets attached.
* Do not submit the fork to WinGet.

### macOS

Primary user install path:

```sh
# Download and extract jedit-X.Y.Z-aarch64-macos.tar.gz or
# jedit-X.Y.Z-x86_64-macos.tar.gz, then run:
./jedit --version
```

Release owner responsibility:

* Confirm the GitHub Release has both macOS archives attached.
* Keep the source-build installer working as a fallback.
* Do not submit the fork to Homebrew.

### Linux

Primary user install path:

```sh
# Download and extract jedit-X.Y.Z-x86_64-linux.tar.gz, then run:
./jedit --version
```

Release owner responsibility:

* Confirm the GitHub Release has the Linux archive attached.
* Keep the source-build installer working as a fallback.
* Do not submit the fork to Snapcraft or distro package repositories.

## Post-Release Verification

After publishing, verify a clean install on each platform:

```powershell
jedit.exe --version
```

```sh
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/philip-flop/edit/main/assets/install.sh | sh
jedit --version
```

```sh
~/.local/bin/jedit --version
```

Record any packaging failures as issues and fix them in a patch release.

## Rollback Policy

Do not delete or retarget published tags. If a release must be withdrawn:

1. Mark the GitHub Release as deprecated in the release notes.
2. Remove or supersede broken release assets where possible.
3. Publish a new patch release with the fix.
