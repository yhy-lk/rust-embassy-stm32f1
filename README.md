<<<<<<< HEAD
# rust-embassy
存放一些embassy项目
=======
# STM32F103 Blue Pill RTC Calendar with OLED Display

![Demo Image](demo.jpg) *示例效果图*

## 项目简介

本项目使用Rust的Embassy框架在STM32F103 Blue Pill开发板上实现了一个万年历系统，复现了B站UP主keysking的万年历项目。

**相关视频链接：**
- [keysking原项目视频](https://www.bilibili.com/video/BV1VkwqeoErY)
- [本项目效果展示](https://www.bilibili.com/video/BV1vdg8zXEen/)

## 硬件要求

### 必需硬件
- STM32F103C8T6 Blue Pill开发板
- SSD1306 OLED显示屏 (128×64)
- 旋转编码器
- 按键开关
- ST-LINK调试器

### 可选硬件
- keysking的开发板（可直接使用）

## 开发环境准备

### 先决条件
1. **STM32开发经验**：需要有STM32单片机C语言开发基础
2. **Rust编程基础**：需要掌握Rust基本语法

### 学习资源
- [Rust官方教程](https://doc.rust-lang.org/book/title-page.html)
- [B站Rust视频教程](https://www.bilibili.com/video/BV1m1sreSEoh)
- [嵌入式Rust书籍](https://doc.rust-lang.org/stable/embedded-book/intro/index.html)
- [Embassy框架教程](https://embassy.dev/book/#_introduction)
- [Embassy STM32 API文档](https://docs.rs/embassy-stm32/latest/embassy_stm32/index.html)

## 快速开始

### 1. 硬件连接
参考`src/calendar.rs`文件顶部的接线说明进行硬件连接。

### 2. 编译运行
```bash
cargo run --bin calendar --release
```

## 模块测试

`examples`目录下包含所有子模块的独立测试代码：

| 模块 | 文件 | 运行命令 |
|------|------|----------|
| LED控制 | `blinky.rs` | `cargo run --bin blinky --release` |
| 按键检测 | `exti.rs` | `cargo run --bin exti --release` |
| OLED显示 | `text_i2c.rs` | `cargo run --bin text_i2c --release` |
| 编码器 | `qei.rs` | `cargo run --bin qei --release` |
| 软件RTC | `rtc.rs` | `cargo run --bin rtc --release` |

**测试步骤：**
1. 将对应示例文件从`examples`复制到`src/bin`目录
2. 执行相应的运行命令

## 技术说明

1. **RTC实现**：由于STM32F1系列RTC功能不完善，本项目采用软件实现的RTC
2. **Embassy框架**：使用异步任务处理多任务需求
3. **硬件抽象**：充分利用Rust的类型系统保证硬件访问安全

## 常见问题

1. **编译错误**：确保已安装正确的工具链和依赖
   ```bash
   rustup target add thumbv7em-none-eabihf
   ```

2. **下载失败**：检查ST-LINK连接和驱动安装

3. **显示异常**：确认OLED显示屏的I2C地址和接线

## 贡献指南

欢迎提交Pull Request或Issue报告问题。贡献前请确保：
- 代码通过rustfmt格式化
- 所有测试用例通过
- 更新相关文档

## 许可证

MIT License
>>>>>>> master
