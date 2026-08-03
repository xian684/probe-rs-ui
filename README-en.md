# Probe-rs Flasher

A graphical embedded firmware flashing tool built on [probe-rs](https://probe.rs/) and [egui](https://github.com/emilk/egui).

It automatically detects debug probes, identifies target chips, supports manual chip selection, and provides one-click flashing, chip erase, and reset — no need to memorize command-line flags.

> 中文文档见 [README.md](README.md)

## Features

- 🔍 **Probe auto-scan**: Enumerates connected debug probes (ST-Link, J-Link, DAPLink / CMSIS-DAP, etc.).
- 🎯 **Target auto-detection**: Auto-identifies the chip when the probe reports it (ST-Link / J-Link, etc.).
- 🔌 **Connection mode**: Choose **Normal** or **Under Reset** connection, matching STM32 BOOT0/BOOT1 boot configurations — useful when target code interferes with SWD or when booting from system memory.
- 🧩 **Manual chip selection**: Full built-in probe-rs chip database with three-level cascade selection by brand, family and variant, plus real-time keyword search; switch between **Built-in pack** and **External pack** views (required for probes such as DAPLink / CMSIS-DAP that cannot self-identify).
- 📦 **Advanced chip config**: Embeds [target-gen](https://probe.rs/docs/tools/target-gen/) on the left — pick a local CMSIS pack (`.pack` / `.pdsc` / `.zip`) to generate chip descriptions, with two modes: **Generate** (files only) or **Generate & Import**. Duplicate families are deduplicated automatically (new variants merged), and existing YAML description files can also be loaded.
- 🌐 **ARM online index**: In Advanced chip config, search the public [Keil.pidx](https://www.keil.com/pack/index.pidx) index (filtered by keyword such as `GD32` / `STM32F4`), pick a Pack and download + generate chip descriptions in one click, auto-imported into the External pack view.
- 📦 **External pack**: Chips imported via YAML or CMSIS pack generation stay **separate from the built-in three-level menu** — pick a family from the dropdown and a variant from the list in the **External pack** view of the manual selection area, then connect by model with one click.
- 📁 **Firmware auto-location**: Pick a project folder and the tool scans common build outputs (cargo `target/`, Keil `Objects/`, CubeIDE `Debug/`, CMake `build/`, etc.) and auto-selects the best firmware; a dropdown lets you switch when multiple candidates are found.
- ⚡ **Flashing**: Supports `.elf` / `.axf` / `.hex` / `.bin` / `.uf2`, including extensionless Rust ELF build artifacts.
- ⚙️ **Configurable options**: chip erase before flash, verify after flash, keep unwritten bytes, reset and run after flash.
- 🗑️ **Chip erase / target reset**: One-click operations.
- 💾 **Read firmware**: Read flash over an address range and export it as `.bin` (auto-filled with the chip's flash region on connect).
- 📊 **Progress display**: Real-time progress bars for erase, program, and verify; operations with an unknown total size (e.g. chip erase) show a spinner.
- 📡 **RTT log monitor**: Disabled by default; once enabled, a bottom panel streams target RTT up-channel output in real time (channels are auto-labeled), and can send data to the target's down channel 0 via Enter or a button.
- 🌐 **Bilingual UI**: Automatically loads an available CJK system font on Windows, macOS, or Linux; switch between 中文 and English from the top bar.

## Requirements

- Windows / Linux / macOS
- Rust 1.97 or newer (to build)
- A SWD / JTAG debug probe (ST-Link, J-Link, DAPLink, CMSIS-DAP, etc.)

## Build

```bash
cargo build --release
```

The executable is produced at `target/release/probe-rs-ui` (`probe-rs-ui.exe` on Windows). You can run it directly.

### Windows package & Release publishing

Every push to `master` builds a `probe-rs-ui-windows-x86_64.zip` with GitHub Actions. Download it from the relevant run's artifacts on the repository [Actions page](https://github.com/xian684/probe-rs-ui/actions/workflows/build-packages.yml). Two ways to publish a Release:

- **Automatic**: push a `v*` tag (e.g. `v1.0.0`) — a GitHub Release is created automatically with the zip attached;
- **Manual**: run the workflow from the Actions page and fill in a version tag (e.g. `v0.2.0`) — it tags and publishes the Release for you.

## Usage

1. Connect the debug probe and target chip. The app scans probes on startup.
2. Click **Auto-detect Target**; if your probe does not support auto-detection (e.g. DAPLink), pick the brand, family and variant under **Manual Target Selection**, then click **Connect by Model**.
3. If the target chip is not in the built-in database, switch to **Advanced chip config** on the left, pick a vendor CMSIS pack (`.pack` / `.pdsc` / `.zip`) and click **Generate & Import** (or load a YAML description file first); then in the manual selection area switch to the **External pack** view, pick the family and variant, and click **Connect by Model**.
4. Pick a firmware file, or click **Select Project Folder...** to auto-locate compiled firmware.
5. Toggle the flashing options as needed, then click **Flash**.

## Dependencies

| Dependency | Version |
| ---------- | ------- |
| eframe / egui | 0.31 |
| probe-rs | 0.32 |
| target-gen | 0.32 |
| rfd | 0.17 |

## Project Structure

```
probe-rs-ui/
├── build.rs         Windows resource script (version info)
└── src/
    ├── main.rs      Entry point: window config, font setup, eframe launch
    ├── app/         App state hub: state/entry, config persistence, event dispatch, flash & memory actions
    ├── panels/      UI panels: top bar, device detection (manual target / advanced chip config), central (flash / memory / RTT / ARM index) and more
    ├── worker/      Background thread: command dispatch, probe connect, flashing, memory R/W, CMSIS Pack, ARM online index
    ├── chips.rs     Built-in chip database & brand grouping
    ├── config.rs    config.toml persistence
    ├── firmware.rs  Firmware scan & format detection
    ├── rtt.rs       RTT session & channel I/O
    ├── fonts.rs     CJK font loading
    └── i18n.rs      Language support (中文/English)
```


**UI / backend communication model**: the UI thread (`app`/`panels`) and the background thread (`worker`) are decoupled through two mpsc channels — `WorkerCommand` sends operations down, `WorkerEvent` reports results up. The probe-rs `Session` lives only in the worker thread, so time-consuming flashing/erasing/RW never blocks the UI.

## FAQ

**Why can't DAPLink auto-detect the chip?**
DAPLink / CMSIS-DAP use a generic debug protocol that does not report the chip identity to the host, so auto-detection is impossible. Select the chip family and variant under **Manual Target Selection** and connect by model instead.

**Firmware not found after picking a project folder?**
Click **Select Project Folder...** to scan common build output directories recursively; if nothing shows up, make sure the firmware is in `.elf` / `.hex` / `.bin` / `.uf2` format.
