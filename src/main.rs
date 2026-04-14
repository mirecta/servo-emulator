//! ESP32-S3 (esp-idf) application
//! - Measures two RC servo pulse widths (1000..2000 µs expected) on two GPIO pins
//! - Displays two horizontal gauges and a numeric pulse width (or \"No Sig\") on an SSD1306 128x64 OLED via I2C
//!
//! Wiring (defaults used in this file):
//! - I2C SDA = GPIO21
//! - I2C SCL = GPIO20
//! - Servo inputs = GPIO4 (servo 0) and GPIO5 (servo 1)
//!
//! Pulse mapping:
//! - A detected pulse width of exactly 0 is treated as \"No Sig\" (no recent pulse).
//! - Pulses in the range 1000..2000 µs map linearly to 0..180°.
//! - Pulses outside that range are clamped to the nearest endpoint.

#![allow(unused_imports)]
#![allow(dead_code)]

use core::sync::atomic::{AtomicI32, Ordering};
use std::ffi::c_void;
use std::time::Duration;

use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use heapless::String;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};

use esp_idf_hal::gpio::Pins;
use esp_idf_hal::i2c::{config::Config as I2cConfig, I2cDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::prelude::*;

use esp_idf_sys as sys;

// Pin and configuration constants
const SDA_GPIO: i32 = 21;
const SCL_GPIO: i32 = 20; // updated per your board
const SERVO_PINS: [i32; 2] = [4, 5];

const PULSE_MIN_US: i32 = 1000;
const PULSE_MAX_US: i32 = 2000;

const DISPLAY_WIDTH: u32 = 128;
const DISPLAY_HEIGHT: u32 = 64;

// Shared atomics for pulse widths (microseconds). 0 => no signal yet.
static PULSE_US: [AtomicI32; 2] = [AtomicI32::new(0), AtomicI32::new(0)];

static LAST_RISING_US: [AtomicI32; 2] = [AtomicI32::new(0), AtomicI32::new(0)];

#[repr(C)]
struct IsrArg {
    gpio_num: i32,
    index: usize,
}

// C-style ISR called by ESP-IDF when GPIO interrupt fires
extern "C" fn gpio_isr_handler(arg: *mut c_void) {
    if arg.is_null() {
        return;
    }
    // Safety: we only pass alive boxed IsrArg pointers in setup
    let isr_arg: &IsrArg = unsafe { &*(arg as *const IsrArg) };
    let gpio = isr_arg.gpio_num;
    let idx = isr_arg.index;

    // read the level
    let level = unsafe { sys::gpio_get_level(gpio) };
    // timestamp in microseconds
    let now = unsafe { sys::esp_timer_get_time() } as i32;

    if level == 1 {
        // rising edge
        LAST_RISING_US[idx].store(now, Ordering::SeqCst);
    } else {
        // falling edge
        let start = LAST_RISING_US[idx].load(Ordering::SeqCst);
        if start > 0 && now >= start {
            let width = now - start;
            // sanity clamp: ignore ridiculously large values
            if width > 0 && width < 200_000 {
                PULSE_US[idx].store(width, Ordering::SeqCst);
            }
        }
    }
}

fn pulse_to_deg_clamped(pulse_us: i32) -> i32 {
    if pulse_us <= 0 {
        return 0;
    }
    if pulse_us <= PULSE_MIN_US {
        return 0;
    }
    if pulse_us >= PULSE_MAX_US {
        return 180;
    }
    let rel = (pulse_us - PULSE_MIN_US) as f32 / (PULSE_MAX_US - PULSE_MIN_US) as f32;
    (rel * 180.0).round() as i32
}

fn draw_gauges<D>(display: &mut D)
where
    D: DrawTarget<Color = BinaryColor>,
{
    // read pulses atomically
    let p0 = PULSE_US[0].load(Ordering::SeqCst);
    let p1 = PULSE_US[1].load(Ordering::SeqCst);

    // convert to degrees (for bar rendering)
    let d0 = pulse_to_deg_clamped(p0);
    let d1 = pulse_to_deg_clamped(p1);

    // Layout geometry
    let margin_x = 8;
    let gauge_width = (DISPLAY_WIDTH as i32) - 2 * (margin_x as i32);
    let gauge_h = 12;

    // Clear (draw background as empty by drawing black rectangle over whole area)
    // For `ssd1306` buffered mode we typically clear buffer before drawing; here we draw an empty rect as a simple approach.
    // However, embedded-graphics has no "clear", so upper layer should ensure buffer cleared. We'll draw over important areas.

    // Servo 0 (top)
    let top_y = 8;
    // border
    let _ = Rectangle::new(
        Point::new(margin_x as i32 - 1, top_y - 1),
        Size::new((gauge_width + 2) as u32, (gauge_h + 2) as u32),
    )
    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
    .draw(display);

    // filled bar for degrees
    let bar_w0 = ((d0 as f32 / 180.0) * (gauge_width as f32)).round() as i32;
    let _ = Rectangle::new(
        Point::new(margin_x as i32, top_y),
        Size::new(bar_w0.max(0) as u32, gauge_h as u32),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(display);

    // Text under gauge
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    // Degree display or No Sig
    if p0 == 0 {
        let mut s: String<32> = String::new();
        let _ = core::fmt::write(&mut s, format_args!("S0: No Sig"));
        let _ = Text::new(
            &s,
            Point::new(margin_x as i32, top_y + gauge_h + 6),
            text_style,
        )
        .draw(display);
    } else {
        let mut sdeg: String<32> = String::new();
        let _ = core::fmt::write(&mut sdeg, format_args!("S0: {}°", d0));
        let _ = Text::new(
            &sdeg,
            Point::new(margin_x as i32, top_y + gauge_h + 6),
            text_style,
        )
        .draw(display);

        let mut spulse: String<32> = String::new();
        let _ = core::fmt::write(&mut spulse, format_args!("{} us", p0));
        let _ = Text::new(
            &spulse,
            Point::new(margin_x as i32 + 72, top_y + gauge_h + 6),
            text_style,
        )
        .draw(display);
    }

    // Servo 1 (bottom)
    let bot_y = 32;
    let _ = Rectangle::new(
        Point::new(margin_x as i32 - 1, bot_y - 1),
        Size::new((gauge_width + 2) as u32, (gauge_h + 2) as u32),
    )
    .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
    .draw(display);

    let bar_w1 = ((d1 as f32 / 180.0) * (gauge_width as f32)).round() as i32;
    let _ = Rectangle::new(
        Point::new(margin_x as i32, bot_y),
        Size::new(bar_w1.max(0) as u32, gauge_h as u32),
    )
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(display);

    if p1 == 0 {
        let mut s: String<32> = String::new();
        let _ = core::fmt::write(&mut s, format_args!("S1: No Sig"));
        let _ = Text::new(
            &s,
            Point::new(margin_x as i32, bot_y + gauge_h + 6),
            text_style,
        )
        .draw(display);
    } else {
        let mut sdeg: String<32> = String::new();
        let _ = core::fmt::write(&mut sdeg, format_args!("S1: {}°", d1));
        let _ = Text::new(
            &sdeg,
            Point::new(margin_x as i32, bot_y + gauge_h + 6),
            text_style,
        )
        .draw(display);

        let mut spulse: String<32> = String::new();
        let _ = core::fmt::write(&mut spulse, format_args!("{} us", p1));
        let _ = Text::new(
            &spulse,
            Point::new(margin_x as i32 + 72, bot_y + gauge_h + 6),
            text_style,
        )
        .draw(display);
    }
}

fn setup_gpio_isr() {
    unsafe {
        // Install ISR service (ignore non-OK result for idempotency)
        let _ = sys::gpio_install_isr_service(0);
    }

    for (idx, &pin) in SERVO_PINS.iter().enumerate() {
        unsafe {
            // Configure pin as input with pull-up and any-edge interrupts
            let mut cfg: sys::gpio_config_t = core::mem::zeroed();
            cfg.pin_bit_mask = (1u64 << pin) as u64;
            cfg.mode = sys::gpio_mode_t_GPIO_MODE_INPUT as i32;
            cfg.pull_up_en = 1;
            cfg.pull_down_en = 0;
            cfg.intr_type = sys::gpio_int_type_t_GPIO_INTR_ANYEDGE as i32;
            let _ = sys::gpio_config(&cfg);

            // Prepare ISR arg which we intentionally leak (lives for program lifetime)
            let boxed = Box::new(IsrArg {
                gpio_num: pin,
                index: idx,
            });
            let raw = Box::into_raw(boxed) as *mut c_void;

            // Register ISR handler
            let res = sys::gpio_isr_handler_add(pin, Some(gpio_isr_handler), raw);
            if res != sys::ESP_OK {
                // Non-fatal: print to console if available
                println!("gpio_isr_handler_add returned {}", res);
            }
        }
    }
}

fn main() -> ! {
    // Ensure esp-idf symbols are linked
    esp_idf_sys::link_patches();

    // Set up ISR handlers for servo input pins
    setup_gpio_isr();

    // Initialize peripherals and I2C for SSD1306
    let peripherals = Peripherals::take().expect("Failed to take peripherals");
    let pins = Pins::new(peripherals.pins);

    // Map the used pins. If your hal version names differ you may need to adapt these conversions.
    // Use pull-up input/output pads for I2C pins
    let sda = pins.gpio21.into_pad_input_output();
    let scl = pins.gpio20.into_pad_input_output();

    // Build I2C config and driver (400kHz)
    let mut cfg = I2cConfig::default();
    cfg.baudrate = 400.kHz().into();
    let i2c = match I2cDriver::new(peripherals.i2c0, sda, scl, &cfg) {
        Ok(driver) => driver,
        Err(e) => {
            println!("Failed to create I2C driver: {:?}", e);
            loop {
                std::thread::sleep(Duration::from_secs(1));
            }
        }
    };

    let interface = I2CDisplayInterface::new(i2c);
    let mut display: Ssd1306<_, GraphicsMode> =
        Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
            .into_buffered_graphics_mode();

    display.init().ok();
    display.clear();
    display.flush().ok();

    // initial static message
    {
        let style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);
        let _ = Text::new("Servo Monitor (S3)", Point::new(8, 10), style).draw(&mut display);
        let _ = Text::new("Waiting for pulses...", Point::new(8, 30), style).draw(&mut display);
        display.flush().ok();
    }

    // Main loop: update display at ~10 Hz
    loop {
        // clear the display buffer before drawing
        display.clear();

        draw_gauges(&mut display);

        // flush to screen
        display.flush().ok();

        std::thread::sleep(Duration::from_millis(100));
    }
}
