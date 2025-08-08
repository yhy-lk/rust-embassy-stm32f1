//! STM32 Blue Pill Basic Blinky Example
//! This is a minimal embedded Rust program that blinks the onboard LED (PC13)
//! on the STM32F103 "Blue Pill" development board. It demonstrates:
//! 1. Basic no_std/no_main setup
//! 2. GPIO output configuration
//! 3. Simple timing with embassy-time
//! 4. Logging with defmt
//!
//! Hardware Connection:
//!   - No external connections needed - uses onboard LED at PC13
//!
//! Expected Behavior:
//!   - Onboard LED will blink at 300ms intervals
//!   - Debug messages will be output via defmt RTT

#![no_std] // Disable Rust standard library (required for embedded)
#![no_main] // Disable standard main interface

use defmt::*; // Formatted logging macros
use embassy_executor::Spawner; // Async executor
use embassy_stm32::{
    gpio::{Level, Output, Speed}, // GPIO types
};
use embassy_time::Timer; // Time-related functionality
use {defmt_rtt as _, panic_probe as _}; // Logging and panic handlers

/// Main application entry point
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize microcontroller peripherals with default configuration
    let p = embassy_stm32::init(Default::default());

    // Print startup message (visible via defmt RTT)
    info!("Hello World!");

    // Configure onboard LED (PC13) as push-pull output
    // Initial state: High (LED off for common anode configuration)
    let mut led = Output::new(
        p.PC13,      // Onboard LED pin
        Level::High, // Initial state
        Speed::Low,  // Suitable speed for simple blinking
    );

    // Main application loop
    loop {
        // Turn LED off and log state
        info!("LED state: high (off)");
        led.set_high();

        // Wait for 300 milliseconds
        Timer::after_millis(300).await;

        // Turn LED on and log state
        info!("LED state: low (on)");
        led.set_low();

        // Wait for 300 milliseconds
        Timer::after_millis(300).await;
    }
}

// Notes:
// 1. The `#[embassy_executor::main]` macro sets up the async runtime
// 2. `defmt_rtt` enables logging over RTT (Real Time Transfer)
// 3. `panic_probe` provides panic handling with defmt integration
// 4. GPIO speed is set to Low as we don't need fast toggling for blinking
