#![no_std] // Disable Rust standard library for embedded target
#![no_main] // Use custom entry point

use embassy_executor::Spawner;
use embassy_stm32::{
    gpio::{Level, Output, Speed},
    timer::qei::{Direction, Qei, QeiPin},
};
use embassy_time::Timer;
use main_cargo::hardware::{gpio_led, traits::Led};
use {defmt_rtt as _, panic_probe as _};

/// Example of reading a quadrature encoder while blinking an LED.
///
/// This example demonstrates:
/// 1. Basic LED blinking using embassy-time delays
/// 2. Reading a quadrature encoder position and direction
/// 3. Using defmt for logging
///
/// Hardware connections:
/// - LED: PC13 (onboard LED)
/// - Encoder:
///   - Channel A: PA8 (TIM1_CH1)
///   - Channel B: PA9 (TIM1_CH2)
///   - Note: Connect PA0 and PC13 with a 1k Ohm resistor
///
/// Run with `cargo run --example encoder_blinky`

/// LED blinking task
#[embassy_executor::task]
async fn blinky(mut led: gpio_led::GpioLed<'static>) {
    loop {
        led.toggle(); // Toggle LED state
        Timer::after_millis(500).await; // 500ms delay
    }
}

/// Main application entry point
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize microcontroller peripherals
    let p = embassy_stm32::init(Default::default());
    defmt::info!("Hello World!");

    // Configure onboard LED (PC13)
    let led_pin = Output::new(
        p.PC13,      // Onboard LED pin
        Level::High, // Initial state (off)
        Speed::Low,  // Suitable speed for blinking
    );

    // Spawn LED blinking task
    defmt::unwrap!(spawner.spawn(blinky(gpio_led::GpioLed::new(led_pin))));

    // Configure quadrature encoder interface using TIM1
    let encoder = Qei::new(
        p.TIM1,                 // Timer peripheral
        QeiPin::new_ch1(p.PA8), // Encoder channel A
        QeiPin::new_ch2(p.PA9), // Encoder channel B
    );

    // Main loop reads encoder position and direction
    loop {
        // Read and display current encoder count
        defmt::info!("cnt = {}", encoder.count());

        // Read and display rotation direction
        match encoder.read_direction() {
            Direction::Downcounting => defmt::info!("Downcounting"),
            Direction::Upcounting => defmt::info!("Upcounting"),
        }

        Timer::after_millis(500).await; // Read every 500ms
    }
}
