//! Draw a 1 bit per pixel black and white image. On a 128x64 SSD1306 display over I2C.
//!
//! Image was created with ImageMagick:
//!
//! This example is for the STM32F103 "Blue Pill" board using I2C1.
//!
//! Wiring connections are as follows for a CRIUS-branded display:
//!
//! ```
//!      Display -> Blue Pill
//! (black)  GND -> GND
//! (red)    +5V -> VCC
//! (yellow) SDA -> PB7
//! (green)  SCL -> PB6
//! ```
//!
//! Run on a Blue Pill with `cargo run --example image_i2c`.

#![no_std] // Disable Rust standard library for embedded target
#![no_main] // Use custom entry point

use cortex_m::asm::nop; // No-operation instruction for empty loops
use cortex_m_rt::entry; // Cortex-M runtime entry macro
use defmt_rtt as _; // defmt logging over RTT

// Conditional imports based on async feature
#[cfg(feature = "async")]
use embassy_stm32::{bind_interrupts, i2c, peripherals};

use embassy_stm32::time::Hertz; // Frequency type
use embedded_graphics::{
    image::{Image, ImageRaw}, // Image rendering types
    pixelcolor::BinaryColor,  // 1-bit color (on/off)
    prelude::*,               // Core traits
};
use panic_probe as _; // Panic handler
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*}; // OLED display driver

/// Main entry point for the application
#[entry]
fn main() -> ! {
    // Initialize microcontroller peripherals
    let p = embassy_stm32::init(Default::default());

    // Configure I2C interrupts when async feature is enabled
    #[cfg(feature = "async")]
    bind_interrupts!(struct Irqs {
        I2C1_EV => i2c::EventInterruptHandler<peripherals::I2C1>;
        I2C1_ER => i2c::ErrorInterruptHandler<peripherals::I2C1>;
    });

    // Initialize I2C peripheral
    #[cfg(feature = "async")]
    let i2c = embassy_stm32::i2c::I2c::new(
        p.I2C1,             // I2C1 peripheral
        p.PB6,              // SCL pin
        p.PB7,              // SDA pin
        Irqs,               // Interrupt handlers
        p.DMA1_CH6,         // TX DMA channel
        p.DMA1_CH7,         // RX DMA channel
        Hertz::khz(400),    // 400kHz I2C speed
        Default::default(), // Default configuration
    );

    // Blocking I2C initialization when async not enabled
    #[cfg(not(feature = "async"))]
    let i2c = embassy_stm32::i2c::I2c::new_blocking(
        p.I2C1,
        p.PB6,
        p.PB7,
        Hertz::khz(400),
        Default::default(),
    );

    // Create display interface and driver
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(
        interface,
        DisplaySize128x64,        // 128x64 pixel display
        DisplayRotation::Rotate0, // No rotation
    )
    .into_buffered_graphics_mode(); // Enable buffered graphics

    // Initialize display
    display.init().unwrap();

    // Load raw image data (1-bit per pixel)
    // Image dimensions must match the raw data (64x64 in this case)
    let raw: ImageRaw<BinaryColor> = ImageRaw::new(
        include_bytes!("./rust.raw"), // Embedded image data
        64,                           // Width in pixels
    );

    // Create image object positioned at (32, 0) - centered horizontally
    let im = Image::new(&raw, Point::new(32, 0));

    // Draw image to display buffer
    im.draw(&mut display).unwrap();

    // Flush buffer to physical display
    display.flush().unwrap();

    // Main loop does nothing but keep the program running
    loop {
        nop() // No-operation instruction to prevent optimization
    }
}

/* Additional Technical Notes:
 *
 * 1. Image Preparation:
 *    - The rust.raw file should be a 1-bit per pixel bitmap
 *    - Can be created with ImageMagick using:
 *      `convert input.png -monochrome -depth 1 gray:output.raw`
 *
 * 2. Display Configuration:
 *    - Uses hardware I2C at 400kHz (fast mode)
 *    - Buffered mode reduces flickering during updates
 *
 * 3. Positioning:
 *    - Image is centered horizontally (128-64)/2 = 32
 *    - Placed at top vertically (y=0)
 *
 * 4. Error Handling:
 *    - Unwraps are used for simplicity in example
 *    - Production code should handle potential errors
 */
