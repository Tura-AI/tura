# Release packages

Each GitHub Release contains packages for different installation needs. Choose
one package type; you normally do not need to download all of them.

## Release notes

The current release is
[Tura 0.1.35](https://github.com/Tura-AI/tura/blob/v0.1.35/docs/changelog/0.1.35.md).
The repository [changelog](../../CHANGELOG.md) links to notes for every
published version.

## GUI-only installers

Files beginning with `tura-gui-only-` install only the native desktop GUI. They
do not include the Tura gateway, CLI, TUI, runtime workers, or command packages.
The GUI must connect to an already running Tura gateway. If Tura is not already
installed, use a full release archive or the `tura-ai` npm package first.

Download the installer for your system from the
[latest GitHub Release](https://github.com/Tura-AI/tura/releases/latest):

| System | Package | Installation |
| --- | --- | --- |
| Windows x64 | `tura-gui-only-<tag>-windows-x64-setup.exe` | Run the setup program. Use the `.msi` variant for managed installation. |
| macOS Intel | `tura-gui-only-<tag>-macos-x64.dmg` | Open the DMG and move Tura GUI to Applications. |
| macOS Apple silicon | `tura-gui-only-<tag>-macos-arm64.dmg` | Open the DMG and move Tura GUI to Applications. |
| Linux x64 | `.AppImage`, `.deb`, or `.rpm` with the `tura-gui-only-<tag>-linux-x64` prefix | Use the format supported by your distribution. |

Linux examples:

```bash
chmod +x tura-gui-only-<tag>-linux-x64.AppImage
./tura-gui-only-<tag>-linux-x64.AppImage

sudo apt install ./tura-gui-only-<tag>-linux-x64.deb
sudo dnf install ./tura-gui-only-<tag>-linux-x64.rpm
```

Start Tura from the full installation so its gateway is available, then open
the GUI-only application. If the gateway runs at a non-default address, provide
that gateway URL in the GUI.

## Full release archives

Files named `tura-<tag>-<platform>.<archive>` contain the complete local release:
the CLI and TUI, GUI executable, gateway and runtime services, provider config,
prompts, and command packages.

Use these when you want a self-contained download without npm:

- `tura-<tag>-linux-x64.tar.gz`
- `tura-<tag>-macos-x64.tar.gz`
- `tura-<tag>-macos-arm64.tar.gz`
- `tura-<tag>-windows-x64.zip`

Extract the archive, then run `target/release/tura` on Linux or macOS, or
`target\release\tura.exe` on Windows.

## npm packages

For the simplest CLI installation with npm:

```bash
npm install --global tura-ai
tura
```

`tura-ai` is the main package. It automatically selects one native platform
package:

- `tura-linux-x64`
- `tura-darwin-x64`
- `tura-darwin-arm64`
- `tura-win32-x64`

The platform `.tgz` files are implementation packages used by npm. They contain
native executables and runtime files, but deliberately exclude the Tauri desktop
installers. Do not use them as GUI installers.

The main package is also mirrored to GitHub Packages as `@tura-ai/tura`.

## Source archives

GitHub automatically adds source code `.zip` and `.tar.gz` files to each
release. These contain the repository source and require a local build. For
source installation instructions, see [Install and Uninstall](install.md).
