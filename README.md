# Servo Emulator — ESP32-S3 (esp-idf) README

This project measures two RC-style servo pulse widths (microseconds) on two GPIO pins and visualizes them on a 128×64 OLED (SSD1306-compatible) as two horizontal gauges plus numeric pulse values. It targets an ESP32-S3 board and uses the esp-idf integration for Rust.

Key behavior
- Pulse = 0 → displayed as "No Sig".
- Pulse 1000..2000 µs → mapped linearly to 0..180° and shown as a gauge.
- Defaults (change in code if you wire differently):
  - I2C SDA = GPIO21
  - I2C SCL = GPIO20
  - Servo input 0 = GPIO4
  - Servo input 1 = GPIO5

Contents
- `Cargo.toml` — Cargo manifest for esp-idf-based build
- `src/main.rs` — Application: GPIO ISR pulse measurement + SSD1306 rendering

Wiring
- OLED (I2C, 128×64, SSD1306-compatible):
  - VCC → 3.3V
  - GND → GND
  - SDA → GPIO21
  - SCL → GPIO20
- Servo pulse inputs (signal only; DO NOT connect servo power rails to the ESP's GPIO):
  - Servo 0 signal → GPIO4
  - Servo 1 signal → GPIO5
- Power servos from an appropriate external power supply if you actually drive servos. This project only reads pulses (inputs) — it does not drive servos.

Prerequisites (host machine)
1. ESP-IDF installed and configured for your platform. Follow the official ESP-IDF install guide for your OS and target (ESP32-S3).
2. Rust toolchain (stable) with cargo available.
3. Rust integration with ESP-IDF (the esp-idf-sys / esp-idf-hal crates require you to export the ESP-IDF environment so Rust can link to the native libs).
4. A serial terminal program to view logs from the device (e.g., a serial monitor).

Note: Ensure your ESP-IDF version is compatible with the `esp-idf-sys` / `esp-idf-hal` crate versions declared in `Cargo.toml`. If you see version mismatch errors when building, either update your local ESP-IDF or align the crate versions.

High-level build & flash workflow
1. Export ESP-IDF environment
   - Run the ESP-IDF export script provided by your ESP-IDF installation to set environment variables in the current shell session. This is required so the Rust ESP-IDF bindings can locate headers/libraries.

2. Build
   - From the repository root:
     - Build in release mode:
       - `cargo build --release`
   - If the build fails due to missing or incompatible ESP-IDF libs, verify the ESP-IDF installation and make sure the ESP-IDF version matches the crate expectations.

3. Flash
   - Use your preferred ESP-IDF-based flashing method or the Rust-flashing helper you're familiar with:
     - Example sequence:
       - Put the board in bootloader / programming mode (if required by your board).
       - Invoke the flash command appropriate to your environment, pointing at the built ELF/firmware artifact in `target/` or using the cargo-based flash helper.
     - The exact flash command depends on your local tooling. If you use the ESP-IDF Python tooling, there's a `flash` command in the ESP-IDF tooling that can write the built firmware to the chip using a selected serial port.
     - If you use a cargo-based flashing helper, it typically wraps the same low-level flashing functionality and can also be used.

4. Serial logs / debugging
   - After flashing, open a serial monitor at 115200 baud (or the speed configured by esp-idf in this project) and observe logs (initialization messages, potential `println!` output).
   - If logs are missing, double-check the serial port and cable, and ensure the board is in the running mode (not in bootloader after flashing).

Editing pins or mapping
- To change pins or the pulse-to-degree mapping:
  - Edit `src/main.rs` constants:
    - `SDA_GPIO`, `SCL_GPIO` — I2C pins
    - `SERVO_PINS` — array of two servo input GPIO numbers
    - `PULSE_MIN_US` and `PULSE_MAX_US` — map these microsecond values to 0..180°
  - Rebuild and flash after changes.

Behavior & notes
- The ISR captures rising and falling edges and computes pulse width using high-resolution timer calls. The main loop reads the last captured values and updates the display at ~10 Hz.
- A pulse value of `0` means "no signal detected yet" and shows `No Sig` on the display.
- Very long pulses (>200,000 µs in the code) are ignored as invalid (safety).

Troubleshooting
- Build failures referring to esp-idf headers or libraries:
  - Re-run the ESP-IDF export script in your shell and ensure the environment variables are present.
  - Verify the ESP-IDF version matches the crate's expectations. Adjust the `esp-idf-sys` / `esp-idf-hal` versions in `Cargo.toml` or update your ESP-IDF installation.
- Display does not show anything:
  - Check I2C wiring (SDA/SCL pins and pull-ups).
  - Verify OLED power rails (3.3V) and ground.
  - Try scanning the I2C bus from your board (if you have a utility) to verify the device address; some OLEDs have an alternate I2C address—if the device does not respond, you may need to adapt the driver initialization (check the SSD1306 crate options).
- No pulses detected:
  - Verify the servo signal generator is actually producing pulses on the signal pins.
  - Use a logic probe or oscilloscope to verify pulses (rising/falling edges and widths).
  - Confirm the input pins configured in `src/main.rs` match your wiring.
- Crate / version mismatch:
  - If linking errors mention functions missing from the ESP-IDF libraries, update either your ESP-IDF installation or the crate versions to match each other.

Safety notes
- Do not power servos from the ESP32's 3.3V rail if the servo current draw is significant. Use a proper external supply and common ground.
- GPIO pins are 3.3V. Avoid driving the pins with higher voltages.

Extending the project
- Add calibration UI: allow setting `PULSE_MIN_US` / `PULSE_MAX_US` from the device via buttons or serial commands.
- Smoothing: add a simple exponential moving average for displayed degree values to make gauges less jumpy.
- Additional logging: add more `println!` calls to help debug input capture and I2C initialization.

If you want, I can:
- Add a small helper script with concrete build and flash commands tailored to your host OS.
- Add an optional serial calibration utility to tune min/max pulse values interactively from the board.

Enjoy — let me know if you want the README expanded with explicit commands for your OS or a helper script to automate build + flash.