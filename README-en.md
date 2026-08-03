# Probe-rs Flasher

A graphical embedded firmware flashing tool built on [probe-rs](https://probe.rs/) and [egui](https://github.com/emilk/egui).

It automatically detects debug probes, identifies target chips, supports manual chip selection, and provides one-click flashing, chip erase, and reset — no need to memorize command-line flags.

> 中文文档见 [README.md](README.md)

## Features

- 🔍 **Probe auto-scan**: Enumerates connected debug probes (ST-Link, J-Link, DAPLink / CMSIS-DAP, etc.).
- 🎯 **Target auto-detection**: Auto-identifies the chip when the probe reports it (ST-Link / J-Link, etc.).
- 🔌 **Connection mode**: Choose **Normal** or **Under Reset** connection, matching STM32 BOOT0/BOOT1 boot configurations — useful when target code interferes with SWD or when booting from system memory.
- 🧩 **Manual chip selection**: Full built-in probe-rs chip database with three-level cascade selection by brand, family and variant, plus real-time keyword search (required for probes such as DAPLink / CMSIS-DAP that cannot self-identify).
- 📦 **Advanced chip config**: Embeds [target-gen](https://probe.rs/docs/tools/target-gen/) on the left — pick a local CMSIS pack (`.pack` / `.pdsc` / `.zip`) to generate chip descriptions, with two modes: **Generate** (files only) or **Generate & Import** (also adds them to the manual selection list). Duplicate families are deduplicated automatically (new variants merged), and existing YAML description files can also be loaded.
- 📁 **Firmware auto-location**: Pick a project folder and the tool scans common build outputs (cargo `target/`, Keil `Objects/`, CubeIDE `Debug/`, CMake `build/`, etc.) and auto-selects the best firmware; a dropdown lets you switch when multiple candidates are found.
- ⚡ **Flashing**: Supports `.elf` / `.axf` / `.hex` / `.bin` / `.uf2`, including extensionless Rust ELF build artifacts.
- ⚙️ **Configurable options**: chip erase before flash, verify after flash, keep unwritten bytes, reset and run after flash.
- 🗑️ **Chip erase / target reset**: One-click operations.
- 💾 **Read firmware**: Read flash over an address range and export it as `.bin` (auto-filled with the chip's flash region on connect).
- 📊 **Progress display**: Real-time progress bars for erase, program, and verify; operations with an unknown total size (e.g. chip erase) show a spinner.
- 📡 **RTT log monitor**: Disabled by default; once enabled, a bottom panel streams target RTT up-channel output in real time (channels are auto-labeled), and can send data to the target's down channel 0 via Enter or a button.
- 🌐 **Bilingual UI**: Automatically loads an available CJK system font on Windows, macOS, or Linux; switch between 中文 and English from the top bar.
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

### Windows package

Every push to `master` builds a `probe-rs-ui-windows-x86_64.zip` with GitHub Actions. Download it from the relevant run's artifacts on the repository [Actions page](https://github.com/xian684/probe-rs-ui/actions/workflows/build-packages.yml). Pushing a `v*` tag (e.g. `v1.0.0`) also creates a GitHub Release with the zip attached.

## Usage

1. Connect the debug probe and target chip. The app scans probes on startup.
2. Click **Auto-detect Target**; if your probe does not support auto-detection (e.g. DAPLink), pick the brand, family and variant under **Manual Target Selection**, then click **Connect by Model**.
3. Pick a firmware file, or click **Select Project Folder...** to auto-locate compiled firmware.
4. Toggle the flashing options as needed, then click **Flash**.

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
├── build.rs                 Build script: Windows resource embedding (icon, version info)
├── assets/
│   └── icon.ico             Windows app icon
└── src/
    ├── main.rs              Entry point: window config, icon generation, font setup, eframe launch
    │
    ├── app/                 App state hub (ProbeUiApp, owned by the UI thread)
    │   ├── mod.rs           State struct, new(), log helpers, eframe::App main loop (update)
    │   ├── settings.rs      Config persistence: apply_config / collect_config (config.toml)
    │   ├── events.rs        Event dispatch: handle_event applies WorkerEvent to UI state
    │   └── actions.rs       Actions: flashing / memory R/W / format detection (parse_hex_bytes)
    │
    ├── panels/              egui panel rendering (all methods on ProbeUiApp)
    │   ├── mod.rs           Panel module declarations
    │   ├── top.rs           Top bar: title, connection status, theme / language switchers
    │   ├── device/          Left device-detection panel
    │   │   ├── mod.rs       Entry: probe picker, connection mode, auto-detect, sub-panel tabs
    │   │   ├── manual.rs    Manual target: search + brand/family/variant cascade + connect
    │   │   ├── target_gen.rs Advanced chip config: CMSIS Pack → chip descriptions (generate / generate & import)
    │   │   └── info.rs      Target info box: chip & memory map after connect
    │   ├── central.rs       Central panel: flash / memory viewer / RTT tabs
    │   ├── flash.rs         Flashing view: file picker, options, progress, read firmware
    │   ├── mem_panel.rs     Memory viewer: arbitrary-address R/W with hex dump
    │   ├── rtt_panel.rs     RTT log view: channel select, send/receive, auto-scroll
    │   └── log.rs           Bottom log panel: global operation log
    │
    ├── worker/              Background thread (probe-rs Session lives only here; UI never blocks)
    │   ├── mod.rs           Public types (WorkerCommand/WorkerEvent) and spawn entry
    │   ├── run.rs           Thread main loop: command dispatch, RTT polling, connect reply
    │   ├── probe.rs         Probe scan, target attach (auto/manual/under-reset), reset
    │   ├── flash.rs         Flash, chip erase, read flash to bin
    │   ├── memory.rs        Memory read/write
    │   ├── progress.rs      Flash progress callback → WorkerEvent progress mapping
    │   └── target_gen.rs    target-gen integration: CMSIS Pack → families → YAML/registry
    │
    ├── chips.rs             Built-in chip database enumeration and brand grouping (BRAND_RULES)
    ├── config.rs            Portable config.toml persistence (next to the exe, hidden attribute)
    ├── firmware.rs          Firmware scanning and format detection (ELF/HEX/BIN/UF2, ELF magic)
    ├── rtt.rs               RTT session lifecycle and channel I/O (called by worker)
    ├── fonts.rs             CJK font loading (candidate paths for Windows/macOS/Linux)
    └── i18n.rs              Language support: Msg enum + MSGS table + placeholder fill (中文/English)
```

**UI / backend communication model**: the UI thread (`app`/`panels`) and the background thread (`worker`) are decoupled through two mpsc channels — `WorkerCommand` sends operations down, `WorkerEvent` reports results up. The probe-rs `Session` lives only in the worker thread, so time-consuming flashing/erasing/RW never blocks the UI.

## FAQ

**Why can't DAPLink auto-detect the chip?**
DAPLink / CMSIS-DAP use a generic debug protocol that does not report the chip identity to the host, so auto-detection is impossible. Select the chip family and variant under **Manual Target Selection** and connect by model instead.

**Firmware not found after picking a project folder?**
Click **Select Project Folder...** to scan common build output directories recursively; if nothing shows up, make sure the firmware is in `.elf` / `.hex` / `.bin` / `.uf2` format.
