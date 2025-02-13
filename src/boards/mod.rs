use embassy_net::StackResources;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_hal::clock::CpuClock;
use esp_hal::delay::Delay;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::ledc::channel::config::PinConfig;
use esp_hal::ledc::channel::ChannelIFace;
use esp_hal::ledc::timer::Timer;
use esp_hal::ledc::timer::TimerIFace;
use esp_hal::ledc::{channel, timer, LSGlobalClkSource, Ledc, LowSpeed};
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::Rtc;
use esp_hal::time::RateExtU32;
use esp_hal::timer::timg::TimerGroup;
use esp_hal::{
    dma_buffers,
    gpio::{Level, Output},
    spi::{
        master::{Config, Spi},
        Mode,
    },
};
use esp_wifi::EspWifiController;
use mipidsi::interface::SpiInterface;
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, TearingEffect};
use mipidsi::Builder;

use crate::board::Board;
use crate::board::{types, Wifi};

macro_rules! singleton {
    ($val:expr, $T:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$T> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

pub fn init() -> Board<types::LedChannel, (), types::DisplayImpl<ST7789>, Wifi, types::RTC> {
    let mut config = esp_hal::Config::default();
    config.cpu_clock = CpuClock::_160MHz;
    let peripherals = esp_hal::init(config);

    // log::info!("running at {}", peripherals.);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let rtc = Rtc::new(peripherals.LPWR);

    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    let lstimer0 = singleton!(
        ledc.timer::<LowSpeed>(timer::Number::Timer1),
        Timer<LowSpeed>
    );
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty5Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: 24u32.kHz(),
        })
        .unwrap();
    let led = Output::new(peripherals.GPIO5, Level::Low);

    let mut channel0 = ledc.channel(channel::Number::Channel0, led);
    channel0
        .configure(channel::config::Config {
            timer: lstimer0,
            duty_pct: 10,

            pin_config: PinConfig::PushPull,
        })
        .unwrap();

    let dc = Output::new(peripherals.GPIO15, Level::Low);
    let sck = peripherals.GPIO18;
    let miso = peripherals.GPIO22;
    let mosi = peripherals.GPIO19;
    let cs = peripherals.GPIO4;

    // Define the reset pin as digital outputs and make it high
    let mut rst = Output::new(peripherals.GPIO6, Level::Low);
    rst.set_high();

    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(2400, 2400);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    // Define the SPI pins and create the SPI interface
    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(60u32.MHz())
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sck)
    .with_mosi(mosi)
    .with_miso(miso)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let cs_output = Output::new(cs, Level::High);

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs_output).unwrap();

    // Define the display interface with no chip select
    let buffer = singleton!([0_u8; 240], [u8; 240]);
    let di = SpiInterface::new(spi_device, dc, buffer);
    // Define the display from the display interface and initialize it
    let mut delay = Delay::new();

    let mut display = Builder::new(ST7789, di)
        .reset_pin(rst)
        .display_size(240, 240)
        .color_order(mipidsi::options::ColorOrder::Rgb)
        .invert_colors(ColorInversion::Inverted)
        // .orientation(Orientation::new().rotate(Rotation::Deg180))
        .init(&mut delay)
        .unwrap();

    // Make the display all
    match display.set_tearing_effect(TearingEffect::Vertical) {
        Ok(_) => log::info!("set_tearing_effect successful"),
        Err(e) => log::info!("set_tearing_effect failed"),
    };
    display.clear(Rgb565::BLACK).unwrap();

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let mut rng = Rng::new(peripherals.RNG);

    let init = &*singleton!(
        esp_wifi::init(timg1.timer0, rng.clone(), peripherals.RADIO_CLK).unwrap(),
        EspWifiController<'static>
    );

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller): (
        esp_wifi::wifi::WifiDevice<'_, esp_wifi::wifi::WifiStaDevice>,
        esp_wifi::wifi::WifiController<'_>,
    ) = esp_wifi::wifi::new_with_mode(&init, wifi, esp_wifi::wifi::WifiStaDevice).unwrap();

    let config = embassy_net::Config::dhcpv4(Default::default());

    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Init network stack
    let (stack, runner): (
        embassy_net::Stack<'_>,
        embassy_net::Runner<'_, esp_wifi::wifi::WifiDevice<'_, esp_wifi::wifi::WifiStaDevice>>,
    ) = embassy_net::new(
        wifi_interface,
        config,
        singleton!(StackResources::<3>::new(), StackResources<3>),
        seed,
    );

    Board::new()
        .backlight(channel0)
        .display(display)
        .wifi(Wifi {
            stack,
            runner,
            controller,
        })
        .rtc(rtc)
}
