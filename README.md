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

High-level build & flash (recommended)
1. Export ESP-IDF environment
   - In each shell where you build/flash, run the ESP-IDF export script provided by your ESP-IDF installation. This sets environment variables so the Rust esp-idf bindings can locate headers and native libraries:
     - Example:
       - source /path/to/esp-idf/export.sh

2. Build & flash in one step (recommended)
   - Use `cargo-espflash` to build and flash in a single command. This is the simplest workflow:
     - cargo espflash --chip esp32s3 --release --monitor /dev/ttyUSB0
   - Replace `/dev/ttyUSB0` with the serial port for your board (on Windows use e.g. `COM3`). The `--monitor` flag opens a serial monitor after flashing.

3. Build only (optional)
   - If you prefer to build first and flash separately:
     - cargo build --release
   - The compiled firmware will be in a `target/*/release/` directory (artifact name / path may vary by platform and toolchain).

4. Flash artifact (optional)
   - If you built separately and have a flashing tool such as `espflash`, you can flash the built artifact:
     - espflash /dev/ttyUSB0 path/to/firmware
   - Adjust the device path and artifact path to match your system.

5. Serial logs / debugging
   - If you used `--monitor` with `cargo-espflash`, the monitor will already be open.
   - Otherwise open a serial terminal at 115200 baud (picocom, minicom, screen, or your preferred tool) to view logs and debug prints:
     - picocom -b 115200 /dev/ttyUSB0
   - If logs do not appear, verify the serial port, cable, and that the board is running (not stuck in bootloader mode).

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
- Add a calibration utility (serial or on-display) to set `PULSE_MIN_US` / `PULSE_MAX_US` interactively.
- Add smoothing (e.g. exponential moving average) to stabilize gauge movement.
- Provide explicit platform-tailored build & flash examples for your OS (Linux / macOS / Windows) showing the exact `cargo-espflash` command to run.

Enjoy — tell me which of the above you want next and I'll add it.