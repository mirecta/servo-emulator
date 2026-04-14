#!/usr/bin/env bash
# NOTE: Make this script executable after checkout:
#   chmod +x flash.sh
#
# flash.sh - Build and flash the servo-emulator to an ESP32-S3 using cargo-espflash or espflash.
#
# Usage:
#   ./flash.sh [PORT] [--no-monitor]
#
# Examples:
#   ./flash.sh /dev/ttyUSB0           # build + flash + open monitor (using cargo-espflash if present)
#   PORT=/dev/ttyUSB0 ./flash.sh      # use $PORT env instead of positional arg
#   ./flash.sh COM3 --no-monitor      # on Windows, omit monitor
#
# Behavior:
# - If `cargo-espflash` is installed it will be used (it builds and flashes in one step).
# - Otherwise the script will `cargo build --release`, try to find a release artifact, and use `espflash` to flash it.
# - Default target chip is esp32s3. Adjust flags in this script if you need a different chip.
#

set -euo pipefail

PROG="$(basename "$0")"

show_help() {
    cat <<EOF
$PROG - Build and flash the servo-emulator to an ESP32-S3

Usage:
  $PROG [PORT] [--no-monitor]

Arguments:
  PORT          Serial port for flashing (e.g. /dev/ttyUSB0, /dev/ttyACM0, COM3).
                If omitted, the script will use the PORT environment variable.
  --no-monitor  Do not open the serial monitor after flashing (default: open monitor if supported).

Environment:
  PORT          You may set PORT in the environment instead of passing it on the command line.

Notes:
  - This script prefers 'cargo-espflash' if available. Install it with 'cargo install cargo-espflash'.
  - Alternatively it will use the 'espflash' CLI if available and an ELF binary is found under target/*/release/.
  - Make sure you have the ESP-IDF environment exported (e.g. source <path-to-esp-idf>/export.sh) before building.
EOF
}

# Parse arguments
PORT_ARG=""
OPEN_MONITOR=1

for arg in "$@"; do
    case "$arg" in
        --help|-h)
            show_help
            exit 0
            ;;
        --no-monitor)
            OPEN_MONITOR=0
            ;;
        *)
            if [[ -z "$PORT_ARG" ]]; then
                PORT_ARG="$arg"
            else
                echo "Unknown extra argument: $arg"
                show_help
                exit 2
            fi
            ;;
    esac
done

# Resolve port
PORT="${PORT_ARG:-${PORT:-}}"

if [[ -z "$PORT" ]]; then
    echo "Error: serial port not specified."
    echo
    show_help
    exit 2
fi

# Default chip
CHIP="esp32s3"

echo "Flashing servo-emulator to ${CHIP} on port: ${PORT}"
echo "Opening monitor after flash: $([[ $OPEN_MONITOR -eq 1 ]] && echo yes || echo no)"
echo

# Use cargo-espflash if available (builds + flashes + optional monitor)
if command -v cargo-espflash >/dev/null 2>&1; then
    echo "Found cargo-espflash -> using it to build & flash"
    if [[ $OPEN_MONITOR -eq 1 ]]; then
        echo "Running: cargo espflash --chip ${CHIP} --release --monitor ${PORT}"
        cargo espflash --chip "${CHIP}" --release --monitor "${PORT}"
    else
        echo "Running: cargo espflash --chip ${CHIP} --release ${PORT}"
        cargo espflash --chip "${CHIP}" --release "${PORT}"
    fi
    exit 0
fi

# If cargo-espflash not available, fall back to espflash (or similar) after building
echo "cargo-espflash not found. Falling back to building with cargo and using 'espflash' if available."

echo "Building release artifact..."
cargo build --release

# Try to find a likely firmware artifact under target/*/release/
# Prefer names that match the binary names used in the project (servo-emulator or servo_emulator)
ARTIFACT=""
# Try common patterns
for pattern in "target/*/release/servo-emulator" "target/*/release/servo_emulator" "target/*/release/servo-emulator.elf" "target/*/release/servo_emulator.elf" "target/*/release/servo-emulator.bin" "target/*/release/servo_emulator.bin"; do
    # Use globbing and pick first match if exists
    matches=( $pattern )
    if [[ ${#matches[@]} -gt 0 ]] && [[ -f "${matches[0]}" ]]; then
        ARTIFACT="${matches[0]}"
        break
    fi
done

# If not found, attempt a more general search
if [[ -z "$ARTIFACT" ]]; then
    echo "Did not find expected artifact by simple patterns, searching target/*/release/*servo* ..."
    # Use a portable loop to avoid external dependencies
    for f in target/*/release/*; do
        # skip if no matches
        [[ -e "$f" ]] || continue
        base="$(basename "$f")"
        case "$base" in
            *servo*|*servo_*)
                if [[ -f "$f" ]]; then
                    ARTIFACT="$f"
                    break
                fi
                ;;
        esac
    done
fi

if [[ -z "$ARTIFACT" ]]; then
    echo "Error: Could not locate release artifact in target/*/release/"
    echo "Please check build output and locate the firmware file (ELF or BIN)."
    exit 3
fi

echo "Found firmware artifact: $ARTIFACT"

# Use espflash if available
if command -v espflash >/dev/null 2>&1; then
    echo "Found 'espflash' -> using it to flash the artifact"
    echo "Running: espflash ${PORT} ${ARTIFACT}"
    espflash "${PORT}" "${ARTIFACT}"
    if [[ $OPEN_MONITOR -eq 1 ]]; then
        echo "Opening serial monitor (115200) on ${PORT}..."
        if command -v picocom >/dev/null 2>&1; then
            picocom -b 115200 "${PORT}"
        elif command -v minicom >/dev/null 2>&1; then
            minicom -D "${PORT}" -b 115200
        elif command -v screen >/dev/null 2>&1; then
            screen "${PORT}" 115200
        else
            echo "No serial monitor tool found (picocom/minicom/screen). Connect a serial terminal manually to ${PORT} at 115200 baud."
        fi
    fi
    exit 0
fi

# If we reach here, neither cargo-espflash nor espflash are present. Explain manual steps.
echo "Error: Neither 'cargo-espflash' nor 'espflash' found in PATH."
echo "Built artifact is at: ${ARTIFACT}"
echo
echo "Please use your preferred flashing tool to write the above artifact to your board."
echo "Common options:"
echo " - Install cargo-espflash (recommended): cargo install cargo-espflash"
echo " - Install espflash CLI and run: espflash <PORT> ${ARTIFACT}"
echo
exit 4
