#![no_std] // 禁用标准库，适用于嵌入式环境
#![no_main] // 禁用标准main入口，使用自定义入口点

use defmt::*; // 嵌入式友好日志框架
use embassy_executor::Spawner; // Embassy异步任务调度器
use embassy_stm32::i2c; // STM32 I2C驱动
use embassy_stm32::time::Hertz; // 频率单位
use {defmt_rtt as _, panic_probe as _}; // 日志和panic处理

// 导入自定义的MPU6050姿态解算模块
use main_cargo::hardware::mpu6050_madgwick_solver::Mpu6050MadgwickSolver;

// 全局姿态变量（使用静态可变变量实现任务间共享数据）
// 注意：在嵌入式环境中，需确保访问的安全性（单写单读模式）
static mut ROLL: f32 = 0.; // 滚转角（度）
static mut PITCH: f32 = 0.; // 俯仰角（度）
static mut YAW: f32 = 0.; // 偏航角（度）

/// 主入口函数
///
/// Embassy执行器的主入口点，初始化硬件并启动异步任务
///
/// # 参数
/// - `_spawner`: 任务生成器，用于创建异步任务
#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // 初始化日志系统
    info!("系统启动!");

    // 初始化STM32外设
    let p = embassy_stm32::init(Default::default());

    // 配置I2C2接口（PB10: SCL, PB11: SDA）
    // 设置I2C时钟频率为100kHz
    let i2c = i2c::I2c::new_blocking(p.I2C2, p.PB10, p.PB11, Hertz(100_000), Default::default());

    // 创建MPU6050数据更新任务
    // 设置采样周期为10ms (100Hz)
    _spawner
        .spawn(mpu6050_update(i2c, embassy_time::Duration::from_millis(10)))
        .unwrap();

    // 主循环 - 定期输出姿态数据
    loop {
        // 安全访问全局姿态变量并输出
        unsafe {
            info!("姿态角 - 滚转: {}, 俯仰: {}, 偏航: {}", ROLL, PITCH, YAW);
        }

        // 每950ms输出一次姿态数据（避免与采样周期同步）
        embassy_time::Timer::after_millis(950).await;
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
    delay: embassy_time::Duration,
) {
    // 创建MPU6050姿态解算器实例
    // sample_period = 10ms / 1000 = 0.01秒 (100Hz)
    // beta = 0.1 (Madgwick滤波器增益)
    let mut imu = Mpu6050MadgwickSolver::new(i2c, delay.as_millis() as f32 / 1000.0, 0.1);

    // 初始化传感器 - 配置量程和滤波器
    imu.init().unwrap();
    info!("MPU6050初始化完成");

    // 执行传感器校准（需保持设备静止水平）
    imu.calibration().await.unwrap();
    info!("传感器校准完成");

    // 输出校准结果
    let acc_offset = imu.get_accel_offset();
    info!(
        "加速度零偏 - X: {}, Y: {}, Z: {}",
        acc_offset.x, acc_offset.y, acc_offset.z
    );

    let gyro_offset = imu.get_gyro_offset();
    info!(
        "陀螺仪零偏 - X: {}, Y: {}, Z: {}",
        gyro_offset.x, gyro_offset.y, gyro_offset.z
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
        unsafe {
            ROLL = roll.to_degrees(); // 滚转角（度）
            PITCH = pitch.to_degrees(); // 俯仰角（度）
            YAW = yaw.to_degrees(); // 偏航角（度）
        }

        // 等待下一个采样周期
        ticker.next().await;
    }
}
