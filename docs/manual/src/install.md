# Installing

You need OxiGuitar first: [its manual](https://mateusrdp.github.io/oxiguitar/) says how to get it. This page is about the MIDI file plugin.

## From a release

Each release on the [releases page](https://github.com/mateusrdp/oxiguitar-midi-file/releases) has a zip for each system (Linux, Windows and macOS; Linux and Windows for x86_64 and ARM64), a `.deb`, an `.rpm` and a Flatpak for Linux, a setup program for Windows that installs for you alone (no administrator password), a `.pkg` for macOS that does the same, and a `SHA256SUMS` file to check the downloads against. Take the file that matches your system.

## Copying the file yourself

A plugin is a single file ending in `.oxp`. Unzip the release for your system and put `midi-file.oxp` in your plugins folder:

| System | Your plugins folder |
| --- | --- |
| Linux | `~/.local/share/oxiguitar/plugins/` |
| macOS | `~/Library/Application Support/OxiGuitar/plugins/` |
| Windows | `%APPDATA%\OxiGuitar\plugins\` |

Start OxiGuitar again. **Tools → Plugins…** lists what it found. The same menu switches a plugin off, and deleting the file removes it.

On macOS, a plugin file you downloaded yourself may be refused as coming from an unverified developer. Clear the download mark and start OxiGuitar again:

```sh
xattr -d com.apple.quarantine ~/Library/Application\ Support/OxiGuitar/plugins/*.oxp
```

A plugin installed with the `.pkg` is not marked this way.

## Linux package managers

The package is called `oxiguitar-midi-file`. It lives in a signed package repository on GitHub Pages that you add once.

Debian, Ubuntu and relatives:

```sh
sudo install -d -m 0755 /etc/apt/keyrings
sudo curl -fsSL https://mateusrdp.github.io/oxiguitar-packages/oxiguitar.gpg -o /etc/apt/keyrings/oxiguitar.gpg
sudo curl -fsSL https://mateusrdp.github.io/oxiguitar-packages/oxiguitar.sources -o /etc/apt/sources.list.d/oxiguitar.sources
sudo apt update
sudo apt install oxiguitar-midi-file
```

Fedora and relatives:

```sh
sudo curl -fsSL https://mateusrdp.github.io/oxiguitar-packages/oxiguitar.repo -o /etc/yum.repos.d/oxiguitar.repo
sudo dnf install oxiguitar-midi-file
```

Normal system updates keep the plugin current from then on.

OxiGuitar's Flatpak is a sandbox that cannot see the system's plugin folders, so it needs the plugin as a Flatpak extension. Flathub comes first because Flatpak takes its runtime from there:

```sh
flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --if-not-exists oxiguitar https://mateusrdp.github.io/oxiguitar-packages/flatpak/oxiguitar.flatpakrepo
flatpak install oxiguitar io.github.mateusrdp.OxiGuitar.Plugin.midi-file
```

`flatpak update` keeps it current and `flatpak remove` takes it away.
