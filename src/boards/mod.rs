use display_interface_spi::SPIInterface;
use embedded_graphics::{pixelcolor::Rgb565, prelude::*};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp_wifi::{
    wifi::{
        utils::create_network_interface, AccessPointInfo, ClientConfiguration, Configuration,
        WifiError, WifiStaDevice,
    },
    wifi_interface::WifiStack,
    EspWifiInitFor,
};
use hal::delay::Delay;
use hal::gpio::AnyPin;
use hal::ledc::channel::config::PinConfig;
use hal::ledc::timer::Timer;
use hal::ledc::{channel, timer, LSGlobalClkSource, Ledc, LowSpeed};
use hal::rng::Rng;
use hal::timer::timg::TimerGroup;
use hal::{
    gpio::{Io, Level, Output},
    prelude::*,
    spi::{master::Spi, SpiMode},
    time,
};
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation, Rotation};
use mipidsi::Builder;
use smoltcp::iface::{SocketSet, SocketStorage};
use smoltcp::wire::DhcpOption;

use crate::board::types;
use crate::board::Board;
use crate::board::SpiScreen;

macro_rules! singleton {
    ($val:expr, $T:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$T> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

const SSID: &str = "Livebox-3580";
const PASSWORD: &str = "";

pub fn init() -> Board<types::LedChannel, (), types::DisplayImpl<ST7789>> {
    let peripherals = hal::init(hal::Config::default());
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    let mut lstimer0 = singleton!(
        ledc.get_timer::<LowSpeed>(timer::Number::Timer0),
        Timer<LowSpeed>
    );
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty5Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: 24u32.kHz(),
        })
        .unwrap();
    let led = Output::new(io.pins.gpio5, Level::Low);

    let mut channel0 = ledc.get_channel(channel::Number::Channel0, led);
    channel0
        .configure(channel::config::Config {
            timer: lstimer0,
            duty_pct: 10,

            pin_config: PinConfig::PushPull,
        })
        .unwrap();

    let dc = Output::new(io.pins.gpio15, Level::Low);
    let sck = io.pins.gpio18;
    let miso = io.pins.gpio22;
    let mosi = io.pins.gpio19;
    let cs = io.pins.gpio4;

    // Define the reset pin as digital outputs and make it high
    let mut rst = Output::new(io.pins.gpio6, Level::Low);
    rst.set_high();

    // Define the SPI pins and create the SPI interface
    let spi: types::DisplaySPI = Spi::new(peripherals.SPI2, 60u32.MHz(), SpiMode::Mode0).with_pins(
        sck,
        mosi,
        miso,
        hal::gpio::NoPin,
    );

    let cs_output = Output::new(cs, Level::High);

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs_output).unwrap();

    // Define the display interface with no chip select
    let di = SPIInterface::new(spi_device, dc);
    // Define the display from the display interface and initialize it
    let mut delay = Delay::new();

    let mut display = Builder::new(ST7789, di)
        .reset_pin(rst)
        .color_order(mipidsi::options::ColorOrder::Rgb)
        .invert_colors(ColorInversion::Inverted)
        .orientation(Orientation::new().rotate(Rotation::Deg180))
        .init(&mut delay)
        .unwrap();

    // Make the display all black
    display.clear(Rgb565::BLACK).unwrap();

    /*
        // wifi:
        log::info!("starting wifi");

        let timg0 = TimerGroup::new(peripherals.TIMG0);
        let init = esp_wifi::init(
            EspWifiInitFor::Wifi,
            timg0.timer0,
            Rng::new(peripherals.RNG),
            peripherals.RADIO_CLK,
        )
            .unwrap();

        let mut wifi = peripherals.WIFI;
        log::info!("starting wifi");

        let mut socket_set_entries: [SocketStorage; 3] = Default::default();

        let (iface, device, mut controller, sockets) =
            create_network_interface(&init, &mut wifi, WifiStaDevice, &mut socket_set_entries).unwrap();
        log::info!("created network iface");

        let now = || time::now().duration_since_epoch().to_millis();
        let wifi_stack = WifiStack::new(iface, device, sockets, now);

        let client_config = Configuration::Client(ClientConfiguration {
            ssid: SSID.try_into().unwrap(),
            password: PASSWORD.try_into().unwrap(),
            ..Default::default()
        });
        let res = controller.set_configuration(&client_config);
        log::info!("wifi_set_configuration returned {:?}", res);

        controller.start().unwrap();
        log::info!("is wifi started: {:?}", controller.is_started());
        log::info!("Start Wifi Scan");

        let res: Result<(heapless::Vec<AccessPointInfo, 10>, usize), WifiError> = controller.scan_n();
        if let Ok((res, _count)) = res {
            log::info!("scan successful :{:?}", _count);

            for ap in res {
                log::info!("{:?}", ap);
            }

        } else if let Err(res) = res {
            log::info!("error scanning :{:?}", res);
        }

        log::info!("{:?}", controller.get_capabilities());
        log::info!("wifi_connect {:?}", controller.connect());

        // wait to get connected
        log::info!("Wait to get connected");
        loop {
            let res = controller.is_connected();
            match res {
                Ok(connected) => {
                    if connected {
                        break;
                    }
                }
                Err(err) => {
                    log::info!("{:?}", err);
                    loop {}
                }
            }
        }
        log::info!("{:?}", controller.is_connected());

    */

    Board::new().backlight(channel0).display(display)
}
