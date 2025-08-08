//! STM32 MPU6050 IMU with Madgwick Filter and OLED Display
//! =============================================================================================
//!
//! Date			Author          Notes
//! 2025-07-20	    YHY             Initial release
//!
//!==============================================================================================
//!
//! This firmware implements an IMU system using:
//! - MPU6050 6-axis motion sensor via I2C2
//! - Madgwick filter for sensor fusion
//! - SSD1306 OLED display (128x64) via I2C1
//!
//! Hardware Connections:
//!   OLED Display -> Blue Pill
//!      GND  -> GND
//!      VCC  -> 5V
//!      SDA  -> PB7 (I2C1)
//!      SCL  -> PB6 (I2C1)
//!
//!   MPU6050 Sensor -> Blue Pill
//!      VCC  -> 3.3V
//!      GND  -> GND
//!      SDA  -> PB11 (I2C2)
//!      SCL  -> PB10 (I2C2)
//!
//! Features:
//! 1. Real-time IMU data acquisition at 100Hz
//! 2. Sensor calibration for offset compensation
//! 3. Madgwick filter for attitude estimation
//! 4. Euler angle conversion (roll, pitch, yaw)
//! 5. OLED display of orientation angles

#![no_std] // 禁用标准库，适用于裸机嵌入式环境
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
use {defmt_rtt as _, panic_probe as _}; // 日志记录和panic处理

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

// 欧拉角数据通道（线程安全的单生产者单消费者通道）
static IMU_CHANNEL: Channel<ThreadModeRawMutex, EulerAngles, 1> = Channel::new();

/// 主入口函数
///
/// Embassy执行器的主入口点，负责：
/// 1. 配置系统时钟（HSE 8MHz + PLL倍频到72MHz）
/// 2. 初始化I2C外设（OLED使用I2C1，MPU6050使用I2C2）
/// 3. 启动传感器数据采集任务
/// 4. 启动OLED显示任务
///
/// # 参数
/// - `_spawner`: 任务生成器，用于创建异步任务
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // 配置系统时钟（使用外部8MHz晶振，通过PLL倍频到72MHz）
    let mut config = embassy_stm32::Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(8_000_000),
            // 开发板使用外部振荡器
            mode: HseMode::Oscillator,
        });
        config.rcc.pll = Some(Pll {
            src: PllSource::HSE,   // PLL时钟源选择HSE
            prediv: PllPreDiv::DIV1, // 预分频系数
            mul: PllMul::MUL9,     // 倍频系数（8MHz * 9 = 72MHz）
        });
        config.rcc.sys = Sysclk::PLL1_P; // 系统时钟源选择PLL输出
        config.rcc.ahb_pre = AHBPrescaler::DIV1; // AHB预分频（72MHz）
        config.rcc.apb1_pre = APBPrescaler::DIV2; // APB1预分频（36MHz）
        config.rcc.apb2_pre = APBPrescaler::DIV1; // APB2预分频（72MHz）
    }
    
    // 初始化外设
    let p = embassy_stm32::init(config);

    // 初始化日志系统
    defmt::info!("系统启动!");

    // 配置I2C2接口（PB10: SCL, PB11: SDA）用于MPU6050
    // 设置I2C时钟频率为400kHz
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

    // 绑定I2C1中断处理函数（用于OLED）
    bind_interrupts!(struct Irqs {
        I2C1_EV => EventInterruptHandler<peripherals::I2C1>;
        I2C1_ER => ErrorInterruptHandler<peripherals::I2C1>;
    });

    // 配置I2C1外设（PB6: SCL, PB7: SDA）用于OLED
    // 设置时钟频率为400kHz
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

    // 启动OLED显示任务（刷新周期100ms）
    _spawner
        .spawn(oled_display(
            oled_i2c,
            IMU_CHANNEL.receiver(),
            embassy_time::Duration::from_millis(100),
        ))
        .unwrap();

    // 主循环（保持系统运行）
    loop {
        embassy_time::Timer::after_secs(1000).await;
    }
}

/// MPU6050传感器数据更新任务
///
/// 此异步任务负责：
/// 1. 初始化MPU6050传感器
/// 2. 执行传感器校准（需保持设备静止3秒）
/// 3. 定期采集传感器数据（100Hz）
/// 4. 使用Madgwick滤波器进行姿态解算
/// 5. 将四元数转换为欧拉角（度）
/// 6. 通过通道发送姿态数据
///
/// # 参数
/// - `i2c`: I2C总线实例（阻塞模式），用于与MPU6050通信
/// - `imu_sender`: 数据发送通道
/// - `delay`: 采样周期时长（10ms）
#[embassy_executor::task]
async fn mpu6050_update(
    i2c: i2c::I2c<'static, embassy_stm32::mode::Blocking>,
    imu_sender: Sender<'static, ThreadModeRawMutex, EulerAngles, 1>,
    delay: embassy_time::Duration,
) {
    // 创建MPU6050姿态解算器实例
    // sample_period = 10ms / 1000 = 0.01秒 (100Hz)
    // beta = 0.1 (Madgwick滤波器增益系数)
    let mut imu = Mpu6050MadgwickSolver::new(i2c, delay.as_millis() as f32 / 1000.0, 0.1);

    // 初始化传感器 - 配置量程和数字滤波器
    imu.init().unwrap();
    defmt::info!("MPU6050初始化完成");

    // 执行传感器校准（需保持设备静止水平放置3秒）
    embassy_time::with_timeout(embassy_time::Duration::from_secs(3), async {
        imu.calibration().await.unwrap();
        defmt::info!("传感器校准完成");
    })
    .await
    .unwrap();

    // 输出校准结果（加速度计和陀螺仪零偏）
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

        // 更新姿态解算（Madgwick滤波）
        let quat = data.update().await.unwrap();

        // 将四元数转换为欧拉角（弧度）
        let (roll, pitch, yaw) = quat.euler_angles();

        // 构造欧拉角数据结构（弧度转角度）
        let euler_angles = EulerAngles {
            yaw: yaw.to_degrees() + 180_f32,     // 偏航角（度）
            roll: roll.to_degrees(),   // 滚转角（度）
            pitch: pitch.to_degrees(), // 俯仰角（度）
        };
        
        // 记录当前时间戳（用于性能分析）
        embassy_time::Instant::now().as_micros();
        
        // 发送姿态数据（先清空通道确保最新数据）
        imu_sender.clear();
        imu_sender.send(euler_angles).await;

        // 等待下一个采样周期
        ticker.next().await;
    }
}

/// OLED显示任务
///
/// 此异步任务负责：
/// 1. 初始化SSD1306 OLED显示屏
/// 2. 配置文本渲染样式
/// 3. 从通道获取欧拉角数据
/// 4. 格式化并显示姿态数据（偏航角、滚转角、俯仰角）
/// 5. 定期刷新显示（10Hz）
///
/// # 参数
/// - `i2c`: I2C总线实例（异步模式），用于OLED通信
/// - `imu_channel`: 数据接收通道
/// - `delay`: 显示刷新周期（100ms）
#[embassy_executor::task]
async fn oled_display(
    i2c: i2c::I2c<'static, embassy_stm32::mode::Async>,
    imu_channel: Receiver<'static, ThreadModeRawMutex, EulerAngles, 1>,
    delay: embassy_time::Duration,
) {
    // 初始化显示接口和控制器（128x64分辨率，无旋转）
    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();

    // 配置文本渲染样式（10x20 ASCII字体）
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(BinaryColor::On) // 单色显示（亮色）
        .build();

    // 创建定时刷新器（100ms间隔）
    let mut ticker = embassy_time::Ticker::every(delay);

    // 显示刷新主循环
    loop {
        // 尝试获取最新的欧拉角数据
        if let Ok(euler_angles) = imu_channel.try_peek() {
            // 清空显示缓冲区
            display.clear_buffer();

            // 格式化三个姿态角度的显示字符串
            let text_yaw = format_euler(String::from_str("yaw  ").unwrap(), euler_angles.yaw);
            let text_roll = format_euler(String::from_str("roll ").unwrap(), euler_angles.roll);
            let text_pitch = format_euler(String::from_str("pitch").unwrap(), euler_angles.pitch);

            // 在OLED上渲染偏航角（第一行）
            Text::with_baseline(&text_yaw, Point::new(-1, 1), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();

            // 在OLED上渲染滚转角（第二行）
            Text::with_baseline(&text_roll, Point::new(-1, 22), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();

            // 在OLED上渲染俯仰角（第三行）
            Text::with_baseline(&text_pitch, Point::new(-1, 43), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();

            // 更新物理显示
            display.flush().unwrap();
        }

        // 等待下一个刷新周期
        ticker.next().await;
    }
}

/// 欧拉角数据结构
///
/// 表示三维空间中的物体方向：
/// - yaw: 偏航角（绕Z轴旋转）
/// - roll: 滚转角（绕X轴旋转）
/// - pitch: 俯仰角（绕Y轴旋转）
/// 所有角度单位为度（°）
#[derive(Clone)]
struct EulerAngles {
    yaw: f32,
    roll: f32,
    pitch: f32,
}

/// 格式化欧拉角显示字符串
///
/// 将角度值格式化为固定宽度字符串：
/// 格式："[标签]: [符号][整数部分].[小数部分]°"
/// 示例："pitch: -12.34°"
///
/// # 参数
/// - `s`: 角度标签（如"yaw", "roll", "pitch"）
/// - `angle`: 角度值（度）
///
/// # 返回
/// 格式化后的字符串（最大长度13字符）
fn format_euler(s: String<5>, angle: f32) -> String<13> {
    let mut buf: String<13> = String::new();
    
    // 格式化基本字符串（不含符号）
    write!(
        &mut buf,
        "{}: {:3}.{:02}",
        s,
        angle.abs() as i32,
        ((angle.abs() * 100_f32) as i32) % 100
    )
    .unwrap();
    
    // 处理负号（替换空格为负号）
    if angle.is_sign_negative() {
        unsafe {
            let bytes = buf.as_bytes_mut();
            bytes[6] = b'-';
        }
    }
    
    buf
}