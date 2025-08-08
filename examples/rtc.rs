#![no_std] // Disable Rust standard library for embedded target
#![no_main] // Use custom entry point

//! Software RTC Implementation Example
//!
//! This example demonstrates:
//! 1. Creating a software-based real-time clock (RTC)
//! 2. Using embassy's executor and channels for task communication
//! 3. Formatting and displaying date/time information
//!
//! The example maintains a virtual clock that increments every second and
//! displays the time through defmt logging.

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike};
use defmt::*;
use embassy_executor::Spawner;
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex,
    channel::{Channel, Sender},
};
use embassy_time::Ticker;
use {defmt_rtt as _, panic_probe as _};

// Channel for sharing RTC data between tasks (3 message capacity)
static RTC_CHANNEL: Channel<ThreadModeRawMutex, NaiveDateTime, 3> = Channel::new();

/// Main application entry point
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize microcontroller peripherals
    embassy_stm32::init(Default::default());
    info!("RTC Example Started");

    // Spawn RTC update task that maintains the virtual clock
    _spawner
        .spawn(rtc_update(
            RTC_CHANNEL.sender(),
            embassy_time::Duration::from_secs(1), // Update every second
        ))
        .unwrap();

    // Main display loop
    loop {
        // Wait for new time update
        let now_time = RTC_CHANNEL.receive().await;

        // Display timestamp in Unix format
        info!("Timestamp: {}", now_time.and_utc().timestamp());

        // Display formatted date/time
        info!(
            "Date: {}-{}-{} Time: {}:{}:{}",
            now_time.year(),
            now_time.month(),
            now_time.day(),
            now_time.hour(),
            now_time.minute(),
            now_time.second()
        );
    }
}

/// RTC Update Task
///
/// Maintains a virtual clock that increments every second and sends updates
/// through a channel.
#[embassy_executor::task]
async fn rtc_update(
    control: Sender<'static, ThreadModeRawMutex, NaiveDateTime, 3>,
    delay: embassy_time::Duration,
) {
    // Initialize to a specific date/time (2025-07-18 19:38:20)
    let mut now = NaiveDate::from_ymd_opt(2025, 7, 18)
        .unwrap()
        .and_hms_opt(19, 38, 20)
        .unwrap();

    // Create a ticker that triggers every specified interval
    let mut ticker = Ticker::every(delay);

    loop {
        // Increment time by 1 second
        now = now
            .checked_add_signed(chrono::Duration::seconds(1))
            .unwrap_or(now);

        // Send updated time through channel
        control.send(now).await;

        // Wait for next tick
        ticker.next().await;
    }
}

/* Implementation Notes:
 *
 * 1. The RTC is entirely software-based and will drift over time
 * 2. For production use, consider using hardware RTC peripherals
 * 3. The channel provides thread-safe communication between tasks
 * 4. chrono provides comprehensive date/time handling
 *
 * To extend this example:
 * - Add time adjustment functionality
 * - Implement alarms or timers
 * - Add display output (OLED/LCD)
 */
