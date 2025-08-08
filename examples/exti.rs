//! STM32 Blue Pill Button-Controlled LED Example
//! This program demonstrates button input handling on the STM32F103 "Blue Pill" board:
//! 1. Configures onboard LED (PC13) as output
//! 2. Sets up PB1 as input with external interrupt
//! 3. Implements button press detection with debouncing
//! 4. Toggles LED state on button press
//!
//! Hardware Connections:
//!   - Onboard LED: PC13 (no external connection needed)
//!   - Button: PB1 (connect to ground when pressed, with pull-up enabled)
//!
//! Expected Behavior:
//!   - LED toggles state on each button press
//!   - Press/release events are logged via defmt RTT
//!   - System status is reported every second

#![no_std] // Required for embedded development
#![no_main] // Bypass standard main function

use defmt_rtt as _; // defmt logging over RTT
use embassy_executor::Spawner;
use embassy_stm32::{
    exti::ExtiInput, // External interrupt handling
    gpio::{Level, Output, Pull, Speed},
};
use embassy_time::{Duration, Timer};
use panic_probe as _; // Panic handler with defmt integration

/// Main application entry point
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize microcontroller peripherals with default configuration
    let p = embassy_stm32::init(Default::default());

    // Configure onboard LED (PC13) as push-pull output
    // Initial state: High (LED off for common anode configuration)
    let led = Output::new(p.PC13, Level::High, Speed::Low);

    // Configure button pin (PB1) with:
    // - Internal pull-up resistor (active when button not pressed)
    // - External interrupt capability
    let button_exti = ExtiInput::new(p.PB1, p.EXTI1, Pull::Up);

    // Spawn button monitoring task
    spawner
        .spawn(button_task(button_exti, led))
        .expect("Failed to spawn button task");

    // Main system monitoring loop
    loop {
        Timer::after(Duration::from_millis(1000)).await;
        defmt::info!("System status: operational");
    }
}

/// Button Monitoring Task
///
/// Responsibilities:
/// 1. Detect button presses with hardware interrupt
/// 2. Implement software debouncing
/// 3. Toggle LED state on valid presses
/// 4. Log button events
#[embassy_executor::task]
async fn button_task(mut button: ExtiInput<'static>, mut led: Output<'static>) {
    // Main button event loop
    loop {
        // Wait for falling edge (button press)
        button.wait_for_falling_edge().await;

        // Debounce delay (50ms)
        Timer::after(Duration::from_millis(50)).await;

        // Verify button is still pressed (debounce check)
        if button.get_level() != Level::Low {
            continue; // Ignore false triggers
        }

        defmt::info!("Button press detected");

        // Toggle LED state
        led.toggle();

        // Wait for button release (rising edge)
        button.wait_for_rising_edge().await;

        defmt::info!("Button release detected");
    }
}

// Implementation Notes:
// 1. Debouncing uses both hardware (EXTI) and software (50ms delay) methods
// 2. The task-based architecture allows for easy expansion
// 3. GPIO speed is set to Low as we don't need fast switching
// 4. Pull-up configuration means button should connect to ground when pressed
