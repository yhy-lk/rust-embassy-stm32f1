//! Print "Hello world!" with "Hello rust!" underneath. Uses the `embedded_graphics` crate to draw
//! the text with a 6x10 pixel font.
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
//! Run on a Blue Pill with `cargo run --example text_i2c`.

#![no_std] // Disable Rust standard library for embedded target
#![no_main] // Use custom entry point

use cortex_m::asm::nop; // No-operation instruction for empty loops
use cortex_m_rt::entry; // Cortex-M runtime entry macro
use defmt_rtt as _; // defmt logging over RTT
use embassy_stm32::time::Hertz; // Frequency type

// Conditional imports based on async feature
#[cfg(feature = "async")]
use embassy_stm32::{bind_interrupts, i2c, peripherals};

use embedded_graphics::{
    mono_font::{
        MonoTextStyleBuilder, // Text style configuration
        ascii::FONT_6X10,     // 6x10 pixel font
    },
    pixelcolor::BinaryColor, // 1-bit color (on/off)
    prelude::*,              // Core traits
    text::{Baseline, Text},  // Text rendering types
};
use panic_probe as _; // Panic handler
use ssd1306::{
    I2CDisplayInterface, // I2C display interface
    Ssd1306,             // SSD1306 driver
    prelude::*,          // Display traits
};

/// Main application entry point
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

    // Configure text style using 6x10 pixel font
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10) // Use 6x10 pixel font
        .text_color(BinaryColor::On) // White text
        .build();

    // Draw first line of text at top-left corner (0,0)
    Text::with_baseline(
        "Hello world!",
        Point::zero(), // Position (0,0)
        text_style,
        Baseline::Top, // Align to top
    )
    .draw(&mut display)
    .unwrap();

    // Draw second line of text 16 pixels below first line
    Text::with_baseline(
        "Hello Rust!",
        Point::new(0, 16), // Position (0,16)
        text_style,
        Baseline::Top,
    )
    .draw(&mut display)
    .unwrap();

    // Flush buffer to physical display
    display.flush().unwrap();

    // Main loop does nothing but keep the program running
    loop {
        nop() // No-operation instruction to prevent optimization
    }
}

/* Implementation Notes:
 *
 * 1. Display Configuration:
 *    - Uses hardware I2C at 400kHz (fast mode)
 *    - Buffered mode reduces flickering during updates
 *
 * 2. Text Rendering:
 *    - FONT_6X10 provides basic ASCII character set
 *    - Baseline::Top aligns text to the top of the character cell
 *    - BinaryColor::On draws white text on black background
 *
 * 3. Positioning:
 *    - First line at (0,0) - top-left corner
 *    - Second line at (0,16) - 16 pixels below first line
 */
