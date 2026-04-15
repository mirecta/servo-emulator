PORT ?= /dev/ttyUSB0

esp32:
	cargo build --release --target xtensa-esp32-espidf

esp32s3:
	cargo build --release --target xtensa-esp32s3-espidf

flash-esp32: esp32
	espflash flash -p $(PORT) target/xtensa-esp32-espidf/release/servo-emulator

flash-esp32s3: esp32s3
	espflash flash -p $(PORT) target/xtensa-esp32s3-espidf/release/servo-emulator

.PHONY: esp32 esp32s3 flash-esp32 flash-esp32s3
