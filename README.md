# THide Hover + UWD2

A lightweight Windows 10/11 Rust tray application that auto-hides the taskbar on hover and integrates UWD2's Insider watermark patching in the same process.

This repository is an independent private project by Saulo Tassara. It is not a fork on GitHub. The taskbar code is based on [amnweb/thide](https://github.com/amnweb/thide); the UWD2 integration is derived from [machineonamission/uwd2](https://github.com/machineonamission/uwd2) and is distributed with its required AGPL-3.0 notice.

For the implementation history, local PC paths, validation evidence, and continuation notes, see [`docs/DEVELOPMENT_HISTORY.md`](docs/DEVELOPMENT_HISTORY.md) and [`docs/LOCAL_CONTEXT.md`](docs/LOCAL_CONTEXT.md).

## Features

- 🎯 **Hide/Show Windows 10/11 Taskbar** - Complete control over taskbar visibility
- 🖱️ **Hover reveal on all four edges** - Bottom, top, left, and right taskbar positions
- ⏱️ **300 ms reveal/conceal delay** - Configured for the final local build
- 🖱️ **System Tray Icon** - Easy access from system tray with menu
- 🧩 **UWD2 integration** - Patch the Insider watermark at startup and after Explorer restarts
- 📋 **Start menu coordination** - Keep the taskbar visible while Start is open
- ⌨️ **CLI Support** - Command-line interface for automation
- ⌨️ **Ctrl+Alt+T** - Global taskbar toggle hotkey
- 🔒 **Single Instance** - Prevents multiple instances from running
- 🎨 **YASB Compatible** - Works with YASB and other custom status bars
- ⚡ **Lightweight** - ~600 KB, minimal resource usage
- 🚀 **No Dependencies** - Self-contained executable with static CRT linking
- 💻 **Windows x64** - The final MSI and runtime behavior were validated locally on x64 Windows

## Download

Get the latest release from the [Releases page](../../releases).

### Available Formats

- **MSI Installer** (Recommended) - Installs to Program Files and adds to PATH automatically
- **Portable ZIP** - Standalone package with executable, LICENSE, and README - no installation needed

### Architectures

- **x64** - Primary target; the final MSI and runtime behavior were validated on this PC
- **ARM64** - The source contains the architecture-specific return opcode, but this snapshot does not claim an ARM64 build, installer, or runtime validation

## Installation

### Option 1: MSI Installer (Recommended)

1. Download the x64 MSI from the [Releases page](../../releases)
2. Double-click the MSI file and follow the installation wizard
3. The application will be installed to `C:\Program Files\THide Hover + UWD2\`
4. **PATH is configured automatically** - you can run `thide` from any command prompt/PowerShell window
5. Start menu shortcut is created automatically

**Uninstall:** Use "Add or Remove Programs" in Windows Settings

### Option 2: Portable ZIP

1. Download the x64 portable ZIP from the [Releases page](../../releases)
2. Extract the ZIP file to any location on your system (e.g., `C:\Tools\thide\`)
3. Run `thide.exe` from the extracted folder - no installation required!
4. The ZIP includes:
   - `thide.exe` - The application
   - `LICENSE.txt` - License information
   - `README-PORTABLE.txt` - Quick start guide
5. (Optional) Add the folder to your PATH to use CLI commands globally

## Usage

### GUI Mode

Double-click `thide.exe` to run in system tray mode:

- The app will hide the taskbar and run in the background
- Look for the icon in your system tray
- Right-click the tray icon to access the menu:
  - **Show Taskbar** - Make taskbar visible
  - **Hide Taskbar** - Hide the taskbar
  - **Toggle Taskbar** - Hide if visible, show if hidden
  - **Re-patch Insider watermark** - Apply UWD2 again without restarting THide
  - **Quit** - Exit and restore taskbar

### CLI Mode

Control the running app from the command line:

```powershell
# If installed via MSI, you can run from anywhere:
thide start

# If using portable exe, run from the directory or add to PATH:
.\thide.exe start

# Show the taskbar (if app is running)
thide show

# Hide the taskbar (if app is running)
thide hide

# Toggle the taskbar state (if app is running)
thide toggle

# Re-apply the UWD2 Insider watermark patch
thide patch-watermark

# Stop the app and restore taskbar
thide stop

# Enable autostart on Windows login
thide enable-autostart

# Disable autostart
thide disable-autostart

# Show help
thide help
```

**Notes:**
- **MSI users**: The `thide` command works from any location (added to PATH automatically)
- **Portable users**: Run `.\thide.exe` from the directory, or add the folder to your PATH manually
- The `start` command launches THide in GUI mode if it's not already running
- Control commands (show/hide/stop) require the GUI app to be running
- Autostart commands use Windows registry

### Autostart

Use the built-in CLI command to add THide to Windows startup:

```powershell
# Enable autostart (adds registry entry)
.\thide.exe enable-autostart

# Disable autostart (removes registry entry)
.\thide.exe disable-autostart
```

This adds an entry to `HKEY_CURRENT_USER\Software\Microsoft\Windows\CurrentVersion\Run`.

## Building from Source

### Prerequisites

- [Rust](https://www.rust-lang.org/) (stable toolchain)
- Windows 10/11
- (Optional) [WiX Toolset](https://wixtoolset.org/) for building MSI installers

### Build Steps

```powershell
# Clone this private repository (requires GitHub access)
git clone https://github.com/SauloTassara/thide-hover-uwd2.git
cd thide-hover-uwd2
cargo build --release
```

### Build the MSI used by this project

The canonical installer is the WiX project under `installer/`:

```powershell
cargo build --release
dotnet build installer\THideHoverUwd2.wixproj -c Release
```

The local MSI is written to `target\installer\THideHoverUwd2.msi`. WiX intermediate files under `installer\obj\` are intentionally ignored.

### Experimental ARM64 source build (not validated in this snapshot)

```powershell
# Add ARM64 target
rustup target add aarch64-pc-windows-msvc

# Build for ARM64
cargo build --release --target aarch64-pc-windows-msvc

# The executable will be at: target\aarch64-pc-windows-msvc\release\thide.exe
```

## Compatibility

- ✅ Windows 11 (Primary target)
- ✅ Windows 10 (Should work)
- ⚠️ Windows on ARM64 (source path exists, but no build/runtime gate was completed for this version)
- ✅ [YASB](https://github.com/amnweb/yasb) (Yet Another Status Bar)
- ✅ Other custom status bars using `Shell_TrayWnd` class name

## UWD2 and licensing

UWD2 resolves the current `shell32.dll` watermark function through Microsoft symbols, caches the PDB data under the user's local application data directory, and patches the running `explorer.exe` process. The integrated source is isolated under `src/uwd2/`.

- THide code: MIT, see [`LICENSE`](LICENSE)
- UWD2-derived code: AGPL-3.0, see [`LICENSE-UWD2.txt`](LICENSE-UWD2.txt)
- Attribution and dependency scope: [`THIRD-PARTY-NOTICES.md`](THIRD-PARTY-NOTICES.md)

## Troubleshooting

### Taskbar won't hide

- Ensure you're running the latest version
- Check if another taskbar tool is interfering
- Try running as administrator (usually not needed)

### App won't start / "Already running" message

- Check system tray - the app might already be running
- Kill any existing `thide.exe` processes in Task Manager

### YASB/Custom status bar disappears

- This should NOT happen - the app filters by process name
- Please report as a bug with details
