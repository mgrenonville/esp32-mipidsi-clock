use hal::gpio::AnyPin;
use hal::ledc::channel::config::PinConfig;
use hal::ledc::timer::Timer;
use hal::ledc::{channel, timer, LSGlobalClkSource, Ledc, LowSpeed};
use hal::{
    gpio::{Io, Level, Output},
    prelude::*,
    spi::{master::Spi, SpiMode},
};

use crate::board::types;
use crate::board::Board;
use crate::board::SpiScreen;

macro_rules! singleton {
    ($val:expr, $T:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$T> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

pub fn init() -> Board<types::LedChannel, SpiScreen<types::DisplaySPI>> {
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

    Board::new().backlight(channel0).screen(SpiScreen {
        dc,
        rst,
        cs_output,
        spi,
    })
}
