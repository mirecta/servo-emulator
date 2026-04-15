use core::sync::atomic::{AtomicI32, Ordering};
use std::ffi::c_void;

use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Arc, Line, PrimitiveStyle};
use embedded_graphics::text::{Alignment, Text};
use heapless::String;

use esp_idf_hal::i2c::{I2cConfig, I2cDriver};
use esp_idf_hal::peripherals::Peripherals;
use esp_idf_hal::units::Hertz;

use esp_idf_sys as sys;

const I2C_ADDR: u8 = 0x3C;
const SERVO_PINS: [i32; 2] = [4, 5];
const PULSE_MIN_US: i32 = 1000;
const PULSE_MAX_US: i32 = 2000;

static PULSE_US: [AtomicI32; 2] = [AtomicI32::new(0), AtomicI32::new(0)];
static mut RISE_TIME: [i32; 2] = [0; 2];

extern "C" fn servo_isr(arg: *mut c_void) {
    let ch = arg as usize;
    let pin = SERVO_PINS[ch];
    let level = unsafe { sys::gpio_get_level(pin) };
    let now = unsafe { sys::esp_timer_get_time() } as i32;
    unsafe {
        if level == 1 {
            RISE_TIME[ch] = now;
        } else {
            let width = now.wrapping_sub(RISE_TIME[ch]);
            if width > 0 && width < 200_000 {
                PULSE_US[ch].store(width, Ordering::Relaxed);
            }
        }
    }
}

fn pulse_to_deg(us: i32) -> i32 {
    if us <= PULSE_MIN_US { return 0; }
    if us >= PULSE_MAX_US { return 180; }
    ((us - PULSE_MIN_US) as f32 / (PULSE_MAX_US - PULSE_MIN_US) as f32 * 180.0) as i32
}

struct Sh1107Fb {
    buf: [u8; 1024],
}

impl Sh1107Fb {
    fn new() -> Self {
        Self { buf: [0; 1024] }
    }

    fn clear(&mut self) {
        self.buf.fill(0);
    }

    fn flush(&self, i2c: &mut I2cDriver) {
        let mut tx = [0u8; 65];
        tx[0] = 0x40;
        for page in 0u8..16 {
            let _ = i2c.write(I2C_ADDR, &[0x00, 0xB0 | page], 1000);
            let _ = i2c.write(I2C_ADDR, &[0x00, 0x00, 0x10], 1000);
            let start = (page as usize) * 64;
            tx[1..65].copy_from_slice(&self.buf[start..start + 64]);
            let _ = i2c.write(I2C_ADDR, &tx, 1000);
        }
    }
}

impl DrawTarget for Sh1107Fb {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<BinaryColor>>,
    {
        for Pixel(point, color) in pixels {
            let x = point.x;
            let y = point.y;
            if x < 0 || x >= 128 || y < 0 || y >= 64 {
                continue;
            }
            let idx = (x as usize / 8) * 64 + (63 - y as usize);
            let bit = x as u8 & 7;
            if color.is_on() {
                self.buf[idx] |= 1 << bit;
            } else {
                self.buf[idx] &= !(1 << bit);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Sh1107Fb {
    fn size(&self) -> Size {
        Size::new(128, 64)
    }
}

fn setup_gpio_isr() {
    unsafe {
        let _ = sys::gpio_install_isr_service(0);
    }
    for (idx, &pin) in SERVO_PINS.iter().enumerate() {
        unsafe {
            let mut cfg: sys::gpio_config_t = core::mem::zeroed();
            cfg.pin_bit_mask = 1u64 << pin;
            cfg.mode = sys::gpio_mode_t_GPIO_MODE_INPUT;
            cfg.pull_up_en = sys::gpio_pullup_t_GPIO_PULLUP_ENABLE;
            cfg.pull_down_en = sys::gpio_pulldown_t_GPIO_PULLDOWN_DISABLE;
            cfg.intr_type = sys::gpio_int_type_t_GPIO_INTR_ANYEDGE;
            let _ = sys::gpio_config(&cfg);
            let _ = sys::gpio_isr_handler_add(pin, Some(servo_isr), idx as *mut c_void);
        }
    }
}

fn init_sh1107(i2c: &mut I2cDriver) {
    let cmds: &[&[u8]] = &[
        &[0x00, 0xAE],
        &[0x00, 0xDC, 0x00],
        &[0x00, 0x81, 0x2F],
        &[0x00, 0x20],
        &[0x00, 0xA1],
        &[0x00, 0xC8],
        &[0x00, 0xA8, 0x7F],
        &[0x00, 0xD3, 0x60],
        &[0x00, 0xD5, 0x51],
        &[0x00, 0xD9, 0x22],
        &[0x00, 0xDB, 0x35],
        &[0x00, 0xB0],
        &[0x00, 0xDA, 0x12],
        &[0x00, 0xA4],
        &[0x00, 0xA6],
        &[0x00, 0xAF],
    ];
    for c in cmds {
        let _ = i2c.write(I2C_ADDR, c, 1000);
    }
}

fn draw_gauge<D: DrawTarget<Color = BinaryColor>>(
    display: &mut D,
    cx: i32,
    cy: i32,
    radius: i32,
    label: &str,
    pulse_us: i32,
) {
    let diameter = (radius * 2) as u32;
    let top_left = Point::new(cx - radius, cy - radius);
    let stroke = PrimitiveStyle::with_stroke(BinaryColor::On, 1);
    let text_style = MonoTextStyle::new(&FONT_6X10, BinaryColor::On);

    // Arc background (top semicircle: 180° to 0° clockwise)
    let _ = Arc::new(top_left, diameter, 180.0.deg(), 180.0.deg())
        .into_styled(stroke)
        .draw(display);

    // Baseline
    let _ = Line::new(
        Point::new(cx - radius, cy),
        Point::new(cx + radius, cy),
    )
    .into_styled(stroke)
    .draw(display);

    // Tick marks at 0, 90, 180 degrees
    let tick_len = 4i32;
    for &servo_deg in &[0i32, 45, 90, 135, 180] {
        let angle = (180.0 - servo_deg as f32) * core::f32::consts::PI / 180.0;
        let ox = (radius as f32 * angle.cos()) as i32;
        let oy = (radius as f32 * angle.sin()) as i32;
        let ix = ((radius - tick_len) as f32 * angle.cos()) as i32;
        let iy = ((radius - tick_len) as f32 * angle.sin()) as i32;
        let _ = Line::new(
            Point::new(cx + ox, cy - oy),
            Point::new(cx + ix, cy - iy),
        )
        .into_styled(stroke)
        .draw(display);
    }

    if pulse_us == 0 {
        // No signal
        let _ = Text::with_alignment(
            "---",
            Point::new(cx, cy - 6),
            text_style,
            Alignment::Center,
        )
        .draw(display);

        let mut s: String<16> = String::new();
        let _ = core::fmt::write(&mut s, format_args!("{}", label));
        let _ = Text::with_alignment(
            &s,
            Point::new(cx, cy + 12),
            text_style,
            Alignment::Center,
        )
        .draw(display);

        let _ = Text::with_alignment(
            "No Sig",
            Point::new(cx, cy + 24),
            text_style,
            Alignment::Center,
        )
        .draw(display);
    } else {
        let deg = pulse_to_deg(pulse_us);

        // Needle
        let angle = (180.0 - deg as f32) * core::f32::consts::PI / 180.0;
        let needle_r = radius - 3;
        let nx = cx + (needle_r as f32 * angle.cos()) as i32;
        let ny = cy - (needle_r as f32 * angle.sin()) as i32;
        let _ = Line::new(Point::new(cx, cy), Point::new(nx, ny))
            .into_styled(stroke)
            .draw(display);

        // Degree inside arc
        let mut s: String<16> = String::new();
        let _ = core::fmt::write(&mut s, format_args!("{}d", deg));
        let _ = Text::with_alignment(
            &s,
            Point::new(cx, cy - 6),
            text_style,
            Alignment::Center,
        )
        .draw(display);

        // Label + raw value below
        let mut s2: String<16> = String::new();
        let _ = core::fmt::write(&mut s2, format_args!("{}", label));
        let _ = Text::with_alignment(
            &s2,
            Point::new(cx, cy + 12),
            text_style,
            Alignment::Center,
        )
        .draw(display);

        let mut s3: String<16> = String::new();
        let _ = core::fmt::write(&mut s3, format_args!("{}us", pulse_us));
        let _ = Text::with_alignment(
            &s3,
            Point::new(cx, cy + 24),
            text_style,
            Alignment::Center,
        )
        .draw(display);
    }
}

fn main() -> ! {
    esp_idf_sys::link_patches();

    let p = Peripherals::take().unwrap();
    let cfg = I2cConfig::new().baudrate(Hertz(400_000));
    let mut i2c = I2cDriver::new(p.i2c0, p.pins.gpio23, p.pins.gpio22, &cfg).unwrap();

    init_sh1107(&mut i2c);

    let mut fb = Sh1107Fb::new();
    fb.flush(&mut i2c);
    setup_gpio_isr();

    loop {
        let p0 = PULSE_US[0].load(Ordering::Relaxed);
        let p1 = PULSE_US[1].load(Ordering::Relaxed);

        fb.clear();
        // Left gauge: center at (32, 26), radius 24
        draw_gauge(&mut fb, 32, 26, 24, "S0", p0);
        // Right gauge: center at (96, 26), radius 24
        draw_gauge(&mut fb, 96, 26, 24, "S1", p1);
        fb.flush(&mut i2c);

        unsafe { sys::vTaskDelay(10); }
    }
}
