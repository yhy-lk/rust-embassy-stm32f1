use ahrs::{Ahrs, AhrsError, Madgwick};
use embassy_time::Ticker;
use embedded_hal::i2c::I2c;
use mpu6050::{Mpu6050, Mpu6050Error, device};
use nalgebra::{UnitQuaternion, Vector3};

/// MPU6050传感器结合Madgwick滤波算法的姿态解算器
///
/// 本结构体封装了MPU6050传感器的操作和Madgwick滤波算法，
/// 提供完整的姿态解算解决方案。包含传感器初始化、校准、
/// 数据采集和姿态解算功能。
///
/// # 泛型参数
/// - `I2C`: 实现`embedded_hal::i2c::I2c`接口的类型，用于与MPU6050通信
pub struct Mpu6050MadgwickSolver<I2C> {
    /// MPU6050传感器实例
    mpu: Mpu6050<I2C>,
    /// Madgwick滤波器实例
    filter: Madgwick<f32>,
    /// 原始加速度计数据（未校准）
    accel_raw: Vector3<f32>,
    /// 加速度计零偏校准值
    accel_offset: Vector3<f32>,
    /// 原始陀螺仪数据（未校准）
    gyro_raw: Vector3<f32>,
    /// 陀螺仪零偏校准值
    gyro_offset: Vector3<f32>,
}

impl<I2C, E> Mpu6050MadgwickSolver<I2C>
where
    I2C: I2c<Error = E>,
{
    /// 创建新的MPU6050姿态解算器实例
    ///
    /// # 参数
    /// - `i2c`: I2C总线实例
    /// - `sample_period`: 采样周期（秒），即滤波器更新频率的倒数
    /// - `beta`: Madgwick滤波器增益系数，控制收敛速度和稳定性
    ///
    /// # 返回值
    /// 初始化后的姿态解算器实例
    pub fn new(i2c: I2C, sample_period: f32, beta: f32) -> Self {
        Self {
            mpu: Mpu6050::new(i2c),
            filter: Madgwick::new(sample_period, beta),
            accel_raw: Vector3::zeros(),
            accel_offset: Vector3::new(0.059909668, -0.022489013, 0.07658446),
            gyro_raw: Vector3::zeros(),
            gyro_offset: Vector3::new(0.11233792, -0.052522425, 0.006111393),
        }
    }

    /// 初始化MPU6050传感器
    ///
    /// 执行以下初始化步骤：
    /// 1. 唤醒传感器并重置配置
    /// 2. 设置陀螺仪量程为±500°/s
    /// 3. 设置加速度计量程为±4g
    /// 4. 配置数字低通滤波器为模式2（加速度计94Hz/陀螺仪98Hz）
    /// 5. 配置加速度计高通滤波器为5Hz
    ///
    /// # 返回值
    /// - `Ok(())`: 初始化成功
    /// - `Err(Mpu6050Error<E>)`: 初始化过程中发生的错误
    pub fn init(&mut self) -> Result<(), Mpu6050Error<E>> {
        let mut delay = embassy_time::Delay;

        // 唤醒传感器并应用默认配置
        self.mpu.init(&mut delay)?;

        // 设置陀螺仪量程（±500°/s）
        self.mpu.set_gyro_range(device::GyroRange::D500)?;

        // 设置加速度计量程（±4g）
        self.mpu.set_accel_range(device::AccelRange::G4)?;

        // 设置数字低通滤波器 - 针对100Hz积分频率
        // 模式2：加速度计94Hz/陀螺仪98Hz
        self.set_dlpf_mode(2)?;

        // 设置加速度计高通滤波器 - 5Hz适合姿态解算
        // 滤除低频噪声，保留有效运动信号
        self.mpu.set_accel_hpf(device::ACCEL_HPF::_5)?;

        Ok(())
    }

    /// 传感器校准方法
    ///
    /// 执行以下校准步骤：
    /// 1. 采集100次传感器数据（间隔10ms）
    /// 2. 计算加速度计和陀螺仪的平均值作为零偏
    /// 3. 针对加速度计Z轴减去1g（重力加速度）
    ///
    /// # 注意
    /// 校准时需保持传感器静止且水平放置
    ///
    /// # 返回值
    /// - `Ok(())`: 校准成功
    /// - `Err(Mpu6050Error<E>)`: 校准过程中发生的错误
    pub async fn calibration(&mut self) -> Result<(), Mpu6050Error<E>> {
        // 初始化累加器
        let mut accel_sum = Vector3::zeros();
        let mut gyro_sum = Vector3::zeros();

        // 校准采样次数（100次）
        const TIMES: u8 = 100;

        // 创建10ms间隔的定时器
        let delay = embassy_time::Duration::from_millis(10);
        let mut ticker = Ticker::every(delay);

        // 循环采集数据
        for _ in 0..TIMES {
            // 累加加速度计原始数据（转换为f32）
            accel_sum += self.mpu.get_acc()?.map(|v| v as f32);

            // 累加陀螺仪原始数据（转换为f32）
            gyro_sum += self.mpu.get_gyro()?.map(|v| v as f32);

            // 等待下一个采样点
            ticker.next().await;
        }

        // 计算加速度计零偏（平均值）
        self.accel_offset = accel_sum / TIMES as f32;

        // 针对重力加速度修正Z轴（减去1g）
        // 假设传感器Z轴向上时受+1g重力
        self.accel_offset.z -= 1.0_f32;

        // 计算陀螺仪零偏（平均值）
        self.gyro_offset = gyro_sum / TIMES as f32;

        Ok(())
    }

    /// 获取传感器最新数据
    ///
    /// 从MPU6050读取最新的加速度计和陀螺仪数据，
    /// 并将原始数据转换为f32格式存储
    ///
    /// # 返回值
    /// - `Ok(&mut Self)`: 成功获取数据，返回自身可变引用
    /// - `Err(Mpu6050Error<E>)`: 数据读取过程中发生的错误
    pub async fn get_data(&mut self) -> Result<&mut Self, Mpu6050Error<E>> {
        // 读取加速度计数据并转换为f32
        self.accel_raw = self.mpu.get_acc()?.map(|v| v as f32);

        // 读取陀螺仪数据并转换为f32
        self.gyro_raw = self.mpu.get_gyro()?.map(|v| v as f32);

        Ok(self)
    }

    /// 更新姿态解算结果
    ///
    /// 使用最新采集的传感器数据和校准参数，
    /// 通过Madgwick算法更新姿态四元数
    ///
    /// # 返回值
    /// - `Ok(&UnitQuaternion<f32>)`: 成功更新，返回姿态四元数引用
    /// - `Err(AhrsError)`: 姿态解算过程中发生的错误
    pub async fn update(&mut self) -> Result<&UnitQuaternion<f32>, AhrsError> {
        // 应用校准参数：陀螺仪数据减去零偏
        let calibrated_gyro = self.gyro_raw - self.gyro_offset;

        // 应用校准参数：加速度计数据减去零偏
        let calibrated_accel = self.accel_raw - self.accel_offset;

        // 更新Madgwick滤波器
        self.filter.update_imu(&calibrated_gyro, &calibrated_accel)
    }

    /// 获取加速度计零偏校准值
    ///
    /// # 返回值
    /// 加速度计的零偏校准向量
    pub fn get_accel_offset(&mut self) -> Vector3<f32> {
        self.accel_offset
    }

    /// 获取陀螺仪零偏校准值
    ///
    /// # 返回值
    /// 陀螺仪的零偏校准向量
    pub fn get_gyro_offset(&mut self) -> Vector3<f32> {
        self.gyro_offset
    }

    /// 设置数字低通滤波器(DLPF)模式
    ///
    /// 配置MPU6050的内部数字低通滤波器，有效值范围0-6
    ///
    /// # 参数
    /// - `dlpf_cfg`: 滤波器配置值（0-6）
    ///
    /// # 滤波器模式参考
    /// | 模式 | 加速度计带宽 | 陀螺仪带宽 | 适用采样率 |
    /// |------|--------------|------------|------------|
    /// | 0    | 260Hz       | 256Hz     | >1kHz      |
    /// | 1    | 184Hz       | 188Hz     | 500Hz      |
    /// | 2    | 94Hz        | 98Hz      | 100-200Hz  |
    /// | 3    | 44Hz        | 42Hz      | 50-100Hz   |
    /// | 4    | 21Hz        | 20Hz      | 20-50Hz    |
    /// | 5    | 10Hz        | 10Hz      | 10-20Hz    |
    /// | 6    | 5Hz         | 5Hz       | <10Hz      |
    ///
    /// # 返回值
    /// - `Ok(())`: 配置成功
    /// - `Err(Mpu6050Error<E>)`: 配置过程中发生的错误
    pub fn set_dlpf_mode(&mut self, dlpf_cfg: u8) -> Result<(), Mpu6050Error<E>> {
        // 确保配置值在有效范围内 (0-6)
        let value = dlpf_cfg & 0x07;

        // 写入CONFIG寄存器(地址0x1A)
        self.mpu.write_byte(0x1A, value)?;

        Ok(())
    }
}
