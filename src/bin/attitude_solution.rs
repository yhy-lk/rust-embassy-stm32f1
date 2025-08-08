#![no_std] // 禁用标准库，适用于嵌入式环境
#![no_main] // 禁用标准main入口，使用自定义入口点

use embassy_executor::Spawner; // Embassy异步任务调度器
use embassy_stm32::{
    bind_interrupts,
    i2c::{self, ErrorInterruptHandler, EventInterruptHandler},
    peripherals,
    time::Hertz,
};
use embassy_sync::{
    blocking_mutex::raw::ThreadModeRawMutex,
    channel::{Channel, Receiver, Sender},
};
use {defmt_rtt as _, panic_probe as _}; // 日志和panic处理

use embedded_graphics::{
    mono_font::{MonoTextStyleBuilder, ascii::FONT_10X20},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

use core::fmt::Write;
use core::str::FromStr;
use heapless::String;

// 导入自定义的MPU6050姿态解算模块
use main_cargo::hardware::mpu6050_madgwick_solver::Mpu6050MadgwickSolver;

static IMU_CHANNEL: Channel<ThreadModeRawMutex, EulerAngles, 1> = Channel::new();

/// 主入口函数
///
/// Embassy执行器的主入口点，初始化硬件并启动异步任务
///
/// # 参数
/// - `_spawner`: 任务生成器，用于创建异步任务
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            // Oscillator for bluepill, Bypass for nucleos.
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            src: PllSource::HSE,
            prediv: PllPreDiv::DIV1,
            mul: PllMul::MUL9,
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.ahb_pre = AHBPrescaler::DIV1;
        config.rcc.apb1_pre = APBPrescaler::DIV2;
        config.rcc.apb2_pre = APBPrescaler::DIV1;
    }
    let p = embassy_stm32::init(config);

    // 初始化日志系统
    defmt::info!("系统启动!");

    // 配置I2C2接口（PB10: SCL, PB11: SDA）
    // 设置I2C时钟频率为100kHz
    let imu_i2c =
        i2c::I2c::new_blocking(p.I2C2, p.PB10, p.PB11, Hertz(400_000), Default::default());

    // 创建MPU6050数据更新任务
    // 设置采样周期为10ms (100Hz)
    _spawner
        .spawn(mpu6050_update(
            imu_i2c,
            IMU_CHANNEL.sender(),
            embassy_time::Duration::from_millis(10),
        ))
        .unwrap();

    // Bind I2C interrupt handlers
    bind_interrupts!(struct Irqs {
        I2C1_EV => EventInterruptHandler<peripherals::I2C1>;
        I2C1_ER => ErrorInterruptHandler<peripherals::I2C1>;
    });

    // Configure I2C peripheral at 400kHz
    let oled_i2c = i2c::I2c::new(
        p.I2C1,
        p.PB6,
        p.PB7,
        Irqs,
        p.DMA1_CH6,
        p.DMA1_CH7,
        Hertz::khz(400),
        Default::default(),
    );

    _spawner
        .spawn(oled_display(
            oled_i2c,
            IMU_CHANNEL.receiver(),
            embassy_time::Duration::from_millis(100),
        ))
        .unwrap();

    loop {
        embassy_time::Timer::after_secs(1000).await;
    }
}

/// MPU6050传感器数据更新任务
///
/// 此异步任务负责：
/// 1. 初始化MPU6050传感器
/// 2. 执行传感器校准
/// 3. 定期采集并解算姿态数据
/// 4. 更新全局姿态变量
///
/// # 参数
/// - `i2c`: I2C总线实例，用于与MPU6050通信
/// - `delay`: 采样周期时长
#[embassy_executor::task]
async fn mpu6050_update(
    i2c: i2c::I2c<'static, embassy_stm32::mode::Blocking>,
    imu_sender: Sender<'static, ThreadModeRawMutex, EulerAngles, 1>,
    delay: embassy_time::Duration,
) {
    // 创建MPU6050姿态解算器实例
    // sample_period = 10ms / 1000 = 0.01秒 (100Hz)
    // beta = 0.1 (Madgwick滤波器增益)
    let mut imu = Mpu6050MadgwickSolver::new(i2c, delay.as_millis() as f32 / 1000.0, 0.1);

    // 初始化传感器 - 配置量程和滤波器
    imu.init().unwrap();
    defmt::info!("MPU6050初始化完成");

    // 执行传感器校准（需保持设备静止水平）
    embassy_time::with_timeout(embassy_time::Duration::from_secs(3), async {
        imu.calibration().await.unwrap();
        defmt::info!("传感器校准完成");
    })
    .await
    .unwrap();

    // 输出校准结果
    let acc_offset = imu.get_accel_offset();
    defmt::info!(
        "加速度零偏 - X: {}, Y: {}, Z: {}",
        acc_offset.x,
        acc_offset.y,
        acc_offset.z
    );

    let gyro_offset = imu.get_gyro_offset();
    defmt::info!(
        "陀螺仪零偏 - X: {}, Y: {}, Z: {}",
        gyro_offset.x,
        gyro_offset.y,
        gyro_offset.z
    );

    // 创建精确的定时采样器（10ms间隔）
    let mut ticker = embassy_time::Ticker::every(delay);

    // 数据采集与解算主循环
    loop {
        // 获取最新传感器数据
        let data = imu.get_data().await.unwrap();

        // 更新姿态解算
        let quat = data.update().await.unwrap();

        // 将四元数转换为欧拉角（弧度）
        let (roll, pitch, yaw) = quat.euler_angles();

        // 安全更新全局姿态变量（弧度转角度）
        let euler_angles = EulerAngles {
            yaw: yaw.to_degrees(),     // 偏航角（度）
            roll: roll.to_degrees(),   // 滚转角（度）
            pitch: pitch.to_degrees(), // 俯仰角（度）
        };
        embassy_time::Instant::now().as_micros();
        imu_sender.clear();
        imu_sender.send(euler_angles).await;

        // 等待下一个采样周期
        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn oled_display(
    i2c: i2c::I2c<'static, embassy_stm32::mode::Async>,
    imu_channel: Receiver<'static, ThreadModeRawMutex, EulerAngles, 1>,
    delay: embassy_time::Duration,
) {
    // Initialize display interface and controller
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();

    // Configure text rendering styles
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On)
        .build();

    let mut ticker = embassy_time::Ticker::every(delay);

    loop {
        if let Ok(euler_angles) = imu_channel.try_peek() {
            display.clear_buffer();

            let text_yaw = format_euler(String::from_str("yaw  ").unwrap(), euler_angles.yaw);
            let text_roll = format_euler(String::from_str("roll ").unwrap(), euler_angles.roll);
            let text_pitch = format_euler(String::from_str("pitch").unwrap(), euler_angles.pitch);

            Text::with_baseline(&text_yaw, Point::new(-1, 1), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();

            Text::with_baseline(&text_roll, Point::new(-1, 22), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();

            Text::with_baseline(&text_pitch, Point::new(-1, 43), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();

            display.flush().unwrap();
        }

        ticker.next().await;
    }
}

#[derive(Clone)]
struct EulerAngles {
    yaw: f32,
    roll: f32,
    pitch: f32,
}

fn format_euler(s: String<5>, angle: f32) -> String<14> {
    let mut buf: String<14> = String::new();
    write!(
        &mut buf,
        "{}: {:3}.{:02} ",
        s,
        angle.abs() as i32,
        ((angle.abs() * 100_f32) as i32) % 100
    )
    .unwrap();
    if angle.is_sign_negative() {
        unsafe {
            let bytes = buf.as_bytes_mut();
            bytes[6] = b'-';
        }
    }
    buf
}
