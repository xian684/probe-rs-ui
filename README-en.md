# Probe-rs Flasher

A graphical embedded firmware flashing tool built on [probe-rs](https://probe.rs/) and [egui](https://github.com/emilk/egui).

It automatically detects debug probes, identifies target chips, supports manual chip selection, and provides one-click flashing, chip erase, and reset — no need to memorize command-line flags.

> 中文文档见 [README.md](README.md)

## Screenshot

![Screenshot](docs/screenshot.png)

## Features

- 🔍 **Probe auto-scan**: Enumerates connected debug probes (ST-Link, J-Link, DAPLink / CMSIS-DAP, etc.).
- 🎯 **Target auto-detection**: Auto-identifies the chip when the probe reports it (ST-Link / J-Link, etc.).
- 🧩 **Manual chip selection**: Full built-in probe-rs chip database with real-time keyword search, grouped by chip family and variant (required for probes such as DAPLink / CMSIS-DAP that cannot self-identify).
- 📁 **Firmware auto-location**: Pick a project folder and the tool scans common build outputs (cargo `target/`, Keil `Objects/`, CubeIDE `Debug/`, CMake `build/`, etc.) and auto-selects the best firmware; a dropdown lets you switch when multiple candidates are found.
- ⚡ **Flashing**: Supports `.elf` / `.axf` / `.hex` / `.bin` / `.uf2`.
- ⚙️ **Configurable options**: chip erase before flash, verify after flash, keep unwritten bytes, reset and run after flash.
- 🗑️ **Chip erase / target reset**: One-click operations.
- 📊 **Progress display**: Real-time progress bars for erase, program, and verify.
- 🌐 **Bilingual UI**: Built-in CJK font loading (Microsoft YaHei / SimSun / PingFang / WenQuanYi); switch between 中文 and English from the top bar.
- 🖼️ **Icons**: Buttons have icons and the window uses a chip-style icon.

## Requirements

- Windows / Linux / macOS
- Rust 1.97 or newer (to build)
- A SWD / JTAG debug probe (ST-Link, J-Link, DAPLink, CMSIS-DAP, etc.)

## Build

```bash
cargo build --release
```

The executable is produced at `target/release/probe-rs-ui` (`probe-rs-ui.exe` on Windows). You can run it directly.

## Usage

1. Connect the debug probe and target chip. The app scans probes on startup.
2. Click **Auto-detect Target**; if your probe does not support auto-detection (e.g. DAPLink), search and select the chip family and variant under **Manual Target Selection**, then click **Connect by Model**.
3. Pick a firmware file, or click **Select Project Folder...** to auto-locate compiled firmware.
4. Toggle the flashing options as needed, then click **Flash**.

## Dependencies

| Dependency | Version |
| ---------- | ------- |
| eframe / egui | 0.31 |
| probe-rs | 0.32 |
| rfd | 0.17 |

## Project Structure

```
src/
├── main.rs    Entry point, window configuration and icon
├── app.rs     egui UI, font loading, event handling
├── worker.rs  Background thread: probe scan, connect, flash, erase, reset, firmware scan
└── i18n.rs    Language support (中文 / English)
```

## FAQ

**Why can't DAPLink auto-detect the chip?**
DAPLink / CMSIS-DAP use a generic debug protocol that does not report the chip identity to the host, so auto-detection is impossible. Select the chip family and variant under **Manual Target Selection** and connect by model instead.

**Firmware not found after picking a project folder?**
Click **Select Project Folder...** to scan common build output directories recursively; if nothing shows up, make sure the firmware is in `.elf` / `.hex` / `.bin` / `.uf2` format.
