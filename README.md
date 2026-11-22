# InstallXS 🚀
> **Universal App Hub & macOS-style Drop-to-Install Manager for Arch Linux**  
> Built with **Rust**, **GPUI**, and **Material Design 3 (Material You)**.

![InstallXS Arch Linux](assets/preview.png)

---

## 💡 What Problem Does InstallXS Solve?

On Arch Linux, software is spread across many formats and locations:
- Official **Pacman** packages (`/usr/bin`, `/usr/share/applications`)
- User-compiled **AUR** packages
- **Flatpak** sandboxed apps
- Standalone **AppImages** downloaded to random folders
- **Tarballs (.tar.gz, .tar.xz, .zip)** extracted into `~/Downloads` or forgotten subdirectories
- **Debian (.deb)** packages extracted manually when AUR isn't available

**InstallXS solves this by giving you a unified macOS/Windows-style experience:**
1. **Unified App Hub**: View every installed app across Pacman, AUR, Flatpak, AppImage, and manual installs in one place.
2. **Mac-style Drop & Install**: Drag & drop any `.AppImage`, `.tar.gz`, `.tar.xz`, `.zip`, `.deb`, or binary directly into the window:
   - Automatically detects format, extracts content, and integrates it.
   - Generates FreeDesktop `.desktop` application launchers with icons.
   - Symlinks executables to `~/.local/bin/` so you can launch them from terminal.
3. **Deep AppCleaner (CleanMyMac style)**: Scans for and selectively purges leftover residual files in `~/.config`, `~/.cache`, `~/.local/share`, `~/.local/state`, and Flatpak `~/.var/app` sandbox folders with exact byte calculation and interactive checklists.
4. **1-Click System & Package Updates**: Live updater querying Pacman/Arch (`checkupdates`), Flatpak remotes, and AUR (`yay`), featuring per-package updating and "Update All".
5. **Discover / App Store**: Search millions of packages from AUR and Flathub with instant 1-click installation.
6. **Storage & Disk Usage Inspector**: Query and inspect exact on-disk installed sizes across Pacman, Flatpaks, AppImages, and local apps, with "Sort by: Disk Size".
7. **Machine Migration & Manifest Backup**: Export full machine application portfolios into `~/installxs-backup.json` along with an auto-generated, executable bash restore script (`~/installxs-restore.sh`).
8. **Native FreeDesktop & MIME Integration**: Registered handler for `.AppImage`, `.tar.gz`, `.deb`, and `.zip` archives.
9. **Standalone Release Updater (App Store Style)**: Track GitHub & GitLab releases directly for standalone software (e.g. Helium Browser). Checks for new stable releases, matches CPU architecture (`x86_64`) and package assets, and executes seamless 1-click in-place updates with zero data loss (all profiles, tabs, cookies, and settings in `~/.config` are 100% preserved).
10. **Modern Material Design 3 Expressive UI**: Powered by Zed's GPU-accelerated **GPUI** and Google's Material 3 design tokens, with live search, source filter chips, and Dark/Light theme switching.

---

## 🛠️ Quick Start

### Run GUI:
```bash
cargo run
# or
installxs
```

### CLI & Headless Usage:
```bash
installxs --help          # Show command options
installxs --list          # List all installed apps and sources
installxs --updates       # Check system package updates
installxs --backup        # Export machine backup bundle & restore script
installxs ./my-app.AppImage # Inspect and install directly
```

---

## 📦 Supported Package Formats

| Format | Automatic Action | Uninstall Behavior |
|---|---|---|
| **`.AppImage`** | Moves to `~/.local/share/installxs/apps/<id>/`, `chmod +x`, extracts icon, registers `.desktop` launcher & `~/.local/bin` symlink | Deletes AppImage, launcher, symlink & database entry |
| **`.tar.gz` / `.tar.xz` / `.zip`** | Unpacks into `~/.local/share/installxs/apps/<id>/`, finds main binary & icon, sets executable permissions, generates `.desktop` launcher & symlink | Recursively deletes all unpacked files, launcher & symlink |
| **`.deb` (Debian Package)** | Extracts `data.tar.*`, configures paths, symlinks binary into `~/.local/bin`, extracts icons & registers launcher | Completely removes unpacked directory & launchers |
| **Executable Binary** | Installs to `~/.local/bin/<id>`, sets `+x`, creates desktop launcher | Removes binary and launcher |
| **`.pkg.tar.zst`** | Detects Arch Linux package and guides user to native `pacman -U` | Managed via native pacman |

---

## 🎨 Technology Stack
- **Language**: Rust 2024 Edition
- **UI Framework**: [GPUI](https://github.com/zed-industries/zed) (GPU-accelerated vector UI)
- **Design System**: [gpui-m3](https://github.com/chrx3/gpui-m3) (Material Design 3 Expressive)
- **Integration**: FreeDesktop `.desktop` spec, XDG Base Directory Specification
