# jedit v20.0.0

This is the first stable `jedit` release. jedit is a fork of Microsoft's Edit,
versioned as `20.0.0` to track upstream Edit `2.0.0` while making fork releases
easy to distinguish. The name was also changed so it could live side by side with `edit`.

This is the first release ever for this and it has not been battle tested.
Along with this release, I am also testing the release system it self.

All of this is to say this is more of a beta release of the release system it self. I want to use it to test installing on other systems as well as learn how GitHub releases work.


## Highlights

* Renamed the executable to `jedit`.
* Added GitHub Release archives for Linux, macOS, and Windows.
* Added a release helper script for publishing future releases.
* Added a source-build installer with `--rc`, `--dev`, and `--system` modes.
* Added a file browser pane with keyboard focus support and close controls.
* Added project-wide Find in Files.
* Added user-defined commands through the Command menu.
* Added Catppuccin themes, live settings reload, and theme color diagnostics.
* Added more syntax highlighting grammars and file associations.
* Added macOS Cmd+C/Cmd+V/Cmd+X support via the Kitty keyboard protocol.
* Added line-editing improvements including duplicate/delete/move line actions,
  comment toggling, and whitespace cleanup on save.

## Downloads

The release workflow publishes these archives:

* `jedit-20.0.0-x86_64-linux.tar.gz`
* `jedit-20.0.0-aarch64-macos.tar.gz`
* `jedit-20.0.0-x86_64-windows.zip`
* `jedit-20.0.0-aarch64-windows.zip`

Extract the archive for your platform and run `jedit` or `jedit.exe`.

## Source Install

macOS and Linux users can also build and install from source with the install
script:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/philip-flop/edit/main/assets/install.sh | sh
```

To install the latest release candidate instead:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/philip-flop/edit/main/assets/install.sh | sh -s -- --rc
```

To install system-wide into `/usr/local/bin`:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/philip-flop/edit/main/assets/install.sh | sh -s -- --system
```

The install script builds from source and requires Rust, a C compiler, ICU, and
`curl` or `wget`.

## Windows Install

Windows users can install the latest release with PowerShell:

```powershell
irm https://raw.githubusercontent.com/philip-flop/edit/main/assets/install.ps1 | iex
```

To install the latest release candidate:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/philip-flop/edit/main/assets/install.ps1))) -Rc
```

The installer downloads the matching Windows zip archive, installs `jedit.exe`
to `%USERPROFILE%\.local\bin`, and adds that directory to the user `PATH`.

You can also download and extract one of the Windows zip archives manually, then
run `jedit.exe`.

## Notes

* This fork is distributed directly through GitHub Releases, not through WinGet,
  Homebrew, Snapcraft, or distro package repositories.
* Windows release archives are built by GitHub Actions and are not code-signed.
