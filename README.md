# ss-led

ESP32-C3 / ESP32-C6 异步 RMT 灯带驱动，支持 WS281x RGB 和自定义时序的 RGBW 灯珠。`no_std`、不使用堆，由调用方提供脉冲缓冲区。

## 使用

需要 Rust stable ≥ 1.95 和 `esp-hal ~1.2.2`。引用时通过 feature 选择芯片：

```toml
[dependencies]
ss-led = { git = "https://github.com/koro33e/ss-led", features = ["esp32c3"] }
```

| 芯片 | feature | 编译目标 |
| --- | --- | --- |
| ESP32-C3 | `esp32c3` | `riscv32imc-unknown-none-elf` |
| ESP32-C6 | `esp32c6` | `riscv32imac-unknown-none-elf` |

默认不选择芯片；使用驱动时必须且只能启用一个芯片 feature。C6 项目将上面的 `esp32c3` 改为 `esp32c6`，应用直接引用的 `esp-hal` 也需选择相同芯片。

驱动初始化与发送示例，完整异步运行时配置见下方例程：

```rust,no_run
# #[cfg(feature = "esp-hal")]
# async fn example() -> Result<(), ss_led::Error> {
use esp_hal::{rmt::{PulseCode, Rmt}, time::Rate};
use ss_led::{LedController, RGB, RgbOrder, buffer_size};

let p = esp_hal::init(esp_hal::Config::default());
let rmt = Rmt::new(p.RMT, Rate::from_mhz(80)).unwrap().into_async();
let frequency = rmt.frequency();
let mut buffer = [PulseCode::end_marker(); buffer_size::<RgbOrder>(8)];
let mut controller = LedController::new(rmt.channel0, p.GPIO8, frequency, RgbOrder::GRB, &mut buffer)?;

controller.write([RGB::RED.dim(32); 8]).await?;
controller.write([RGB::BLACK; 8]).await?;
# Ok(())
# }
```

- `write` 接受数组、切片或迭代器；`write_pixel` 仅发送一个像素。
- RGBW 使用 `RGBW`、`RgbwOrder` 和 `buffer_size::<RgbwOrder>(灯珠数)`。
- `with_timing` 接受纳秒单位的 `Timing`。默认使用 `WS2812B_TIMING`，也提供 `WS2812B_V5_TIMING`；请按灯珠规格选择。
- 缓冲区需要 `灯珠数 × 通道数 × 8 + 2` 个 `PulseCode`，每个占 4 字节。容量不足时返回错误，不启动发送。
- `defmt` 启用日志格式支持，应用需提供日志后端。不选择芯片时，可在主机使用颜色、时序常量和容量计算 API。

## 完整例程

[ws2812b](examples/ws2812b.rs)：8 颗 WS2812B 灯珠，使用 GRB 顺序和 `WS2812B_TIMING`，轮流显示红、绿、蓝和熄灭。

例程包含启动入口、Embassy 运行时和 panic 处理。默认 GPIO8 接灯带 DIN，共地；按实际硬件修改引脚、灯珠数量、字节顺序和时序。灯带使用符合其规格的独立电源，必要时加电平转换。

```sh
# ESP32-C3
cargo build --locked --release --example ws2812b --features esp32c3 --target riscv32imc-unknown-none-elf
# ESP32-C6
cargo build --locked --release --example ws2812b --features esp32c6 --target riscv32imac-unknown-none-elf
# 安装 espflash 后，连接对应开发板烧录运行
cargo run --locked --release --example ws2812b --features esp32c3 --target riscv32imc-unknown-none-elf
```

每帧末尾发送低电平锁存信号，默认 300 µs。发送失败或取消后，先成功调用 `reset().await`，再重发完整画面；`reset` 不会清屏。驱动使用一块 RMT RAM，长帧需要异步任务及时回填，应在实际无线、Flash 等负载下验证波形连续性。

## 开发

```sh
cargo test --locked
cargo check --locked --features defmt
cargo fmt --all -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo check --locked --release --lib --features esp32c3,defmt --target riscv32imc-unknown-none-elf
```

测试覆盖颜色顺序、亮度、逐位编码、时序量化、空帧、容量边界和溢出；硬件传输需在开发板上验证。

MIT License，见 [LICENSE](https://github.com/koro33e/ss-led/blob/HEAD/LICENSE)。
