#![no_std]
#![no_main]

// WS2812B LEDs use GRB byte order and a low-level reset pulse after each frame.
use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_hal::{rmt::PulseCode, rmt::Rmt, time::Rate, timer::timg::TimerGroup};
use esp_println as _;
use ss_led::{LedController, RGB, RgbOrder, WS2812B_TIMING, buffer_size};

esp_bootloader_esp_idf::esp_app_desc!();

const LED_COUNT: usize = 8;

#[esp_rtos::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let timer = TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timer.timer0, peripherals.FROM_CPU_INTR0);

    let rmt = Rmt::new(peripherals.RMT, Rate::from_mhz(80))
        .unwrap()
        .into_async();
    let frequency = rmt.frequency();
    let mut buffer = [PulseCode::end_marker(); buffer_size::<RgbOrder>(LED_COUNT)];
    // Connect GPIO8 to WS2812B DIN; adjust the pin and LED_COUNT for your hardware.
    let mut controller = LedController::with_timing(
        rmt.channel0,
        peripherals.GPIO8,
        frequency,
        RgbOrder::GRB,
        &mut buffer,
        WS2812B_TIMING,
    )
    .unwrap();

    loop {
        for color in [RGB::RED, RGB::GREEN, RGB::BLUE, RGB::BLACK] {
            // An array, slice or iterator can supply the frame.
            let frame = [color.dim(32); LED_COUNT];
            controller.write(frame.as_slice()).await.unwrap();
            Timer::after_millis(500).await;
        }
    }
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo<'_>) -> ! {
    esp_println::println!("{info}");
    loop {
        core::hint::spin_loop();
    }
}
