//! STM32F103 Blue Pill RTC Calendar with OLED Display
//! =============================================================================================
//!
//! Date			Author          Notes
//! 20/7/2025	    YHY             Initial release
//!
//!==============================================================================================
//!
//! This firmware implements a calendar/clock system using:
//! - SSD1306 OLED display (128x64) via I2C1
//! - Rotary encoder for time adjustment
//! - Tactile button for mode switching
//!
//! Hardware Connections:
//!   OLED Display -> Blue Pill
//!      GND  -> GND
//!      VCC  -> 5V
//!      SDA  -> PB7
//!      SCL  -> PB6
//!
//!   Rotary Encoder:
//!      CLK  -> PA8 (TIM1_CH1)
//!      DT   -> PA9 (TIM1_CH2)
//!      SW   -> PB15 (with pull-up)
//!
//! Features:
//! 1. Real-time clock with date and weekday display
//! 2. Time adjustment interface with visual cursor
//! 3. Rotary encoder for value modification
//! 4. Button for field selection
//! 5. Onboard LED heartbeat indicator

#![no_std]
#![no_main]

use chrono::{Datelike, NaiveDate, NaiveDateTime, Timelike, Weekday};
use core::fmt::Write;
use defmt_rtt as _; // Global logger
use embassy_executor::Spawner;
use embassy_stm32::{
    bind_interrupts,
    exti::ExtiInput,
    gpio::{Level, Output, Pull, Speed},
    i2c::{self, ErrorInterruptHandler, EventInterruptHandler},
    peripherals,
    time::Hertz,
    timer::qei::{Qei, QeiPin},
};
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex,
    channel::{Channel, Receiver, Sender},
};
use embassy_time::{Ticker, Timer};
use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_8X13, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::{Line, PrimitiveStyle},
    text::{Baseline, Text},
};
use heapless::String;
use panic_probe as _; // Panic handler
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

// Channel for sharing RTC data between tasks
static RTC_CHANNEL: Channel<ThreadModeRawMutex, NaiveDateTime, 2> = Channel::new();

// Channel for rotary encoder delta values
static ARE_CHANNEL: Channel<ThreadModeRawMutex, i32, 3> = Channel::new();

// Channel for button press events (field selection)
static KEY_CHANNEL: Channel<ThreadModeRawMutex, i32, 1> = Channel::new();

/// Main application entry point
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialize peripherals with default configuration
    let p = embassy_stm32::init(Default::default());

    // Bind I2C interrupt handlers
    bind_interrupts!(struct Irqs {
        I2C1_EV => EventInterruptHandler<peripherals::I2C1>;
        I2C1_ER => ErrorInterruptHandler<peripherals::I2C1>;
    });

    // Configure I2C peripheral at 400kHz
    let i2c = i2c::I2c::new(
        p.I2C1,
        p.PB6,
        p.PB7,
        Irqs,
        p.DMA1_CH6,
        p.DMA1_CH7,
        Hertz::khz(400),
        Default::default(),
    );

    // Configure rotary encoder via TIM1 quadrature interface
    let encoder = Qei::new(p.TIM1, QeiPin::new_ch1(p.PA8), QeiPin::new_ch2(p.PA9));

    // Configure button with external interrupt (pull-up configuration)
    let key_exti = ExtiInput::new(p.PB15, p.EXTI15, Pull::Up);

    // Spawn OLED display task
    _spawner
        .spawn(oled_display(
            i2c,
            RTC_CHANNEL.receiver(),
            KEY_CHANNEL.receiver(),
            embassy_time::Duration::from_millis(100), // Refresh every 100ms
        ))
        .unwrap();

    // Spawn RTC update task
    _spawner
        .spawn(rtc_update(
            RTC_CHANNEL.sender(),
            KEY_CHANNEL.receiver(),
            ARE_CHANNEL.receiver(),
            embassy_time::Duration::from_millis(30), // Update interval
        ))
        .unwrap();

    // Spawn rotary encoder processing task
    _spawner
        .spawn(are_update(
            encoder,
            ARE_CHANNEL.sender(),
            embassy_time::Duration::from_millis(100), // Polling interval
        ))
        .unwrap();

    // Spawn button processing task
    _spawner
        .spawn(key_update(
            key_exti,
            KEY_CHANNEL.sender(),
            embassy_time::Duration::from_millis(10), // Debounce interval
        ))
        .unwrap();

    // Configure onboard LED (PC13) as heartbeat indicator
    let mut led = Output::new(p.PC13, Level::High, Speed::Low);
    let mut ticker = Ticker::every(embassy_time::Duration::from_millis(500));

    // Main heartbeat loop - blinks onboard LED
    loop {
        led.set_low(); // LED on
        ticker.next().await;
        led.set_high(); // LED off
        ticker.next().await;
    }
}

/// OLED Display Rendering Task
///
/// Responsibilities:
/// 1. Manage SSD1306 display interface
/// 2. Render date/time information
/// 3. Handle setting mode cursor
/// 4. Implement blinking cursor effect
#[embassy_executor::task]
async fn oled_display(
    i2c: i2c::I2c<'static, embassy_stm32::mode::Async>,
    rtc_channel: Receiver<'static, ThreadModeRawMutex, NaiveDateTime, 2>,
    key_channel: Receiver<'static, ThreadModeRawMutex, i32, 1>,
    delay: embassy_time::Duration,
) {
    let mut ticker = Ticker::every(delay);
    ticker.next().await; // Initial delay

    // Initialize display interface and controller
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();

    // Configure text rendering styles
    let year_month_day_style = MonoTextStyleBuilder::new()
        .font(&FONT_8X13)
        .text_color(BinaryColor::On)
        .build();

    let hour_minute_second_style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    let weekday_style = MonoTextStyleBuilder::new()
        .font(&FONT_8X13)
        .text_color(BinaryColor::On)
        .build();

    // Cursor state management
    let mut cursor_visible = false;
    let mut last_blink_time = embassy_time::Instant::now();
    const BLINK_INTERVAL: embassy_time::Duration = embassy_time::Duration::from_millis(500);

    // Field positions for cursor rendering
    const CURSOR_POSITIONS: [(Point, Point); 6] = [
        // Year: (start, end)
        (Point::new(24, 18), Point::new(24 + 4 * 8, 18)),
        // Month
        (Point::new(24 + 5 * 8, 18), Point::new(24 + 7 * 8, 18)),
        // Day
        (Point::new(24 + 8 * 8, 18), Point::new(24 + 10 * 8, 18)),
        // Hour
        (Point::new(24, 40), Point::new(24 + 2 * 10, 40)),
        // Minute
        (Point::new(24 + 3 * 10, 40), Point::new(24 + 5 * 10, 40)),
        // Second
        (Point::new(24 + 6 * 10, 40), Point::new(24 + 8 * 10, 40)),
    ];

    let mut now = rtc_channel.receive().await; // Initial time value
    let mut set_pos = 0; // Current selected field (0 = no selection)

    loop {
        display.clear_buffer();

        // Update cursor blink state
        if embassy_time::Instant::now() - last_blink_time >= BLINK_INTERVAL {
            cursor_visible = !cursor_visible;
            last_blink_time = embassy_time::Instant::now();
        }

        // Receive updated time if available
        if let Ok(new_time) = rtc_channel.try_receive() {
            now = new_time;
        }

        // Check for field selection changes
        if let Ok(new_pos) = key_channel.try_peek() {
            set_pos = new_pos;
        }

        // Draw cursor if in setting mode and blink state is visible
        if cursor_visible && set_pos != 0 && set_pos <= 6 {
            let (start, end) = CURSOR_POSITIONS[set_pos as usize - 1];
            Line::new(start, end)
                .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 1))
                .draw(&mut display)
                .unwrap();
        }

        // Render date (YYYY-MM-DD)
        let mut date_buf: String<10> = String::new();
        write!(
            &mut date_buf,
            "{:04}-{:02}-{:02}",
            now.year(),
            now.month(),
            now.day()
        )
        .unwrap();
        Text::with_baseline(
            &date_buf,
            Point::new(24, 4),
            year_month_day_style,
            Baseline::Top,
        )
        .draw(&mut display)
        .unwrap();

        // Render time (HH:MM:SS)
        let mut time_buf: String<8> = String::new();
        write!(
            &mut time_buf,
            "{:02}:{:02}:{:02}",
            now.hour(),
            now.minute(),
            now.second()
        )
        .unwrap();
        Text::with_baseline(
            &time_buf,
            Point::new(24, 21),
            hour_minute_second_style,
            Baseline::Top,
        )
        .draw(&mut display)
        .unwrap();

        // Render weekday (centered)
        let weekday_str = match now.weekday() {
            Weekday::Mon => "Monday",
            Weekday::Tue => "Tuesday",
            Weekday::Wed => "Wednesday",
            Weekday::Thu => "Thursday",
            Weekday::Fri => "Friday",
            Weekday::Sat => "Saturday",
            Weekday::Sun => "Sunday",
        };
        let text_width = weekday_str.len() * 8;
        let x_pos = (128 - text_width) / 2;
        Text::with_baseline(
            weekday_str,
            Point::new(x_pos as i32, 46),
            weekday_style,
            Baseline::Top,
        )
        .draw(&mut display)
        .unwrap();

        // Update physical display
        display.flush().unwrap();

        // Wait for next render cycle
        ticker.next().await;
    }
}

/// Software RTC Management Task
///
/// Responsibilities:
/// 1. Maintain virtual real-time clock
/// 2. Handle time adjustments from rotary encoder
/// 3. Manage field selection states
#[embassy_executor::task]
async fn rtc_update(
    rtc_sender: Sender<'static, ThreadModeRawMutex, NaiveDateTime, 2>,
    key_receiver: Receiver<'static, ThreadModeRawMutex, i32, 1>,
    are_receiver: Receiver<'static, ThreadModeRawMutex, i32, 3>,
    delay: embassy_time::Duration,
) {
    // Initialize to a specific date/time (2025-07-18 19:38:20)
    let mut now = NaiveDate::from_ymd_opt(2025, 7, 20)
        .unwrap()
        .and_hms_opt(18, 00, 00)
        .unwrap();

    let mut ticker = Ticker::every(delay);
    let mut set_pos: i32 = 0; // Current selected field
    let mut prev_time = now; // For change detection

    loop {
        // Check for field selection changes
        if let Ok(new_pos) = key_receiver.try_peek() {
            set_pos = new_pos;
        }

        // Apply rotary encoder adjustments based on selected field
        if set_pos != 0 {
            if let Ok(delta) = are_receiver.try_receive() {
                now = match set_pos {
                    1 => now
                        .checked_add_signed(chrono::Duration::days(365 * delta as i64))
                        .unwrap_or(now),
                    2 => now
                        .checked_add_signed(chrono::Duration::days(30 * delta as i64))
                        .unwrap_or(now),
                    3 => now
                        .checked_add_signed(chrono::Duration::days(delta as i64))
                        .unwrap_or(now),
                    4 => now
                        .checked_add_signed(chrono::Duration::hours(delta as i64))
                        .unwrap_or(now),
                    5 => now
                        .checked_add_signed(chrono::Duration::minutes(delta as i64))
                        .unwrap_or(now),
                    6 => now
                        .checked_add_signed(chrono::Duration::seconds(delta as i64))
                        .unwrap_or(now),
                    _ => now,
                };
            }
        } else {
            // Normal time progression
            now = now
                .checked_add_signed(chrono::Duration::milliseconds(delay.as_millis() as i64))
                .unwrap_or(now);
        }

        // Broadcast time updates when changed
        if prev_time != now {
            rtc_sender.clear();
            rtc_sender.send(now).await;
            prev_time = now;
        }

        ticker.next().await;
    }
}

/// Rotary Encoder Processing Task
///
/// Responsibilities:
/// 1. Read encoder position changes
/// 2. Handle counter overflow/underflow
/// 3. Apply smoothing to encoder inputs
/// 4. Broadcast relative changes
#[embassy_executor::task]
async fn are_update(
    encoder: Qei<'static, peripherals::TIM1>,
    are_sender: Sender<'static, ThreadModeRawMutex, i32, 3>,
    delay: embassy_time::Duration,
) {
    let mut ticker = Ticker::every(delay);
    let mut prev_count = encoder.count(); // Last known encoder position
    let mut accumulated_delta = 0; // Smoothed delta value
    const SMOOTHING_FACTOR: i32 = 4; // Sensitivity adjustment

    // Initialization signal
    are_sender.send(0).await;

    loop {
        let curr_count = encoder.count();
        let raw_delta = curr_count as i32 - prev_count as i32;

        // Handle 16-bit counter overflow
        let adjusted_delta = if raw_delta > 32767 {
            raw_delta - 65536 // Forward overflow correction
        } else if raw_delta < -32768 {
            raw_delta + 65536 // Reverse overflow correction
        } else {
            raw_delta
        };

        // Accumulate small movements
        accumulated_delta -= adjusted_delta;

        // Send significant movements (after smoothing)
        if accumulated_delta.abs() >= SMOOTHING_FACTOR {
            are_sender.send(accumulated_delta / SMOOTHING_FACTOR).await;
            accumulated_delta %= SMOOTHING_FACTOR;
        }

        prev_count = curr_count;
        ticker.next().await;
    }
}

/// Button Processing Task
///
/// Responsibilities:
/// 1. Detect button presses with debouncing
/// 2. Cycle through setting modes (year → month → day → hour → minute → second → normal)
/// 3. Broadcast mode changes
#[embassy_executor::task]
async fn key_update(
    mut button: ExtiInput<'static>,
    key_sender: Sender<'static, ThreadModeRawMutex, i32, 1>,
    debounce_delay: embassy_time::Duration,
) {
    let mut current_mode = 0; // 0 = normal, 1-6 = setting modes

    loop {
        // Wait for button press (falling edge)
        button.wait_for_falling_edge().await;

        // Apply debounce delay
        Timer::after(debounce_delay).await;

        // Verify button is still pressed (anti-glitch)
        if button.is_high() {
            continue;
        }

        // Cycle through modes (0 → 1 → 2 → 3 → 4 → 5 → 6 → 0)
        current_mode = (current_mode + 1) % 7;

        // Broadcast new mode
        key_sender.clear();
        key_sender.send(current_mode).await;

        // Wait for button release
        button.wait_for_rising_edge().await;
    }
}
