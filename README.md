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
3. **1-Click Clean Uninstall**: Records an exact file manifest for every managed app. Uninstalling completely removes the binary, all extracted files, desktop launcher, and symlinks without leaving orphan files behind.
4. **Native System Icons & Deduplication**: Resolves real system icons from `/usr/share/pixmaps` and FreeDesktop icon themes, and deduplicates local/system apps.
5. **Modern Material Design 3 Expressive UI**: Powered by Zed's GPU-accelerated **GPUI** and Google's Material 3 design tokens, with live search and Dark/Light theme switching.

---

## 🛠️ Quick Start

### Run with Cargo:
```bash
cargo run
```

### Or run directly from terminal:
```bash
installxs
```

### Install a package via CLI:
```bash
installxs ~/Downloads/my-app.AppImage
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
