# Flumen
# Flumen DAW

Flumen is a modern Digital Audio Workstation (DAW) built with Rust, focusing on performance and a modular architecture.

## Technologies
- **Language**: Rust
- **Graphics**: WGPU (WebGPU)
- **GUI**: egui
- **Audio Engine**: Custom engine built with `cpal`

## Project Structure
- `crates/flumen-engine`: Core audio processing and graph logic.
- `crates/flumen-gui`: WGPU + egui based graphical interface.
- `crates/flumen-common`: Shared data structures and utilities.

## Как запустить проект

### 1. Установка Rust
Убедитесь, что у вас установлен Rust. Если нет, скачайте его с [rustup.rs](https://rustup.rs/).

### 2. Сборка и запуск (Debug)
Для быстрой проверки и разработки используйте:
```bash
cargo run
```
*Примечание: Первая сборка может занять время, так как скачиваются зависимости.*

### 3. Сборка для работы (Release)
Чтобы программа работала быстро и занимала меньше места:
```bash
cargo run --release
```
Файл будет находиться в `target/release/flumen-gui.exe`.

### 4. Очистка места (ВАЖНО)
Если проект занимает слишком много места (6 ГБ+), это связано с временными файлами в `target/`. Чтобы вернуть место, выполните:
```bash
cargo clean
```
После этого проект будет весить всего несколько мегабайт. При следующем запуске `cargo run` папка создастся снова.

## Features
- **Favus**: Pattern-based arrangement.
- **Piano Roll**: Detailed MIDI editing.
- **Omnia FX**: Integrated multi-FX processor (Distortion, EQ, Delay, Reverb).
- **Fluctus Synth**: Built-in multi-waveform synthesizer.