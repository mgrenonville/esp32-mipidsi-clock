use ds1307::Ds1307;
use embassy_sync::{blocking_mutex::raw::NoopRawMutex, mutex::Mutex};
use esp_hal::{gpio::Output, i2c::master::I2c, rtc_cntl::Rtc, tsens::TemperatureSensor};

pub mod types {
    use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
    use esp_hal::gpio::Output;
    use esp_hal::ledc::channel::Channel;
    use esp_hal::spi::master::SpiDmaBus;
    use mipidsi::interface::SpiInterface;

    use esp_hal::ledc::LowSpeed;
    use esp_hal::Async;
    use mipidsi::Display;

    use super::RtcRelated;

    // pub type SPI =  peripherals.SPI2,
    pub type DisplaySPI = SpiDmaBus<'static, Async>;

    pub type RTCUtils = RtcRelated;
    pub type LedChannel = Channel<'static, LowSpeed>;
    pub type DisplayImpl<M> = Display<
        SpiInterface<
            'static,
            ExclusiveDevice<DisplaySPI, Output<'static>, NoDelay>,
            Output<'static>,
        >,
        M,
        Output<'static>,
    >;
}
#[macro_export]
macro_rules! singleton {
    ($val:expr, $T:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$T> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

// https://github.com/SlimeVR/SlimeVR-Rust/blob/main/firmware/src/peripherals/mod.rs
pub struct SpiScreen<SPI> {
    pub dc: Output<'static>,
    pub rst: Output<'static>,
    pub cs_output: Output<'static>,
    pub spi: SPI,
}

pub struct RtcRelated {
    pub ds1307: Mutex<NoopRawMutex, Ds1307<I2c<'static, esp_hal::Blocking>>>,
    pub rtc: Rtc<'static>,
    pub temperature_sensor: TemperatureSensor<'static>,
}

pub struct Wifi {
    pub stack: embassy_net::Stack<'static>,
    pub runner: embassy_net::Runner<
        'static,
        esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>,
    >,
    pub controller: esp_wifi::wifi::WifiController<'static>,
}

pub struct Board<Backlight = (), ScreenSpi = (), Display = (), Wifi = (), RTCUtils = ()> {
    pub screen_backlight: Backlight,
    pub screen_spi: ScreenSpi,
    pub display: Display,
    pub wifi: Wifi,
    pub rtc: RTCUtils,
    // _lifetime: PhantomData<&'d mut Backlight>,
}

impl Board {
    pub fn new() -> Self {
        Self {
            screen_backlight: (),
            screen_spi: (),
            display: (),
            wifi: (),
            rtc: (),
        }
    }
}

/// Type-level destructors for `Board` which turn peripheral type into () to solve partial move.
impl<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
    Board<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
{
    pub fn backlight_peripheral(
        self,
    ) -> (Backlight, Board<(), ScreenSpi, Display, Wifi, RTCUtils>) {
        (
            self.screen_backlight,
            Board {
                screen_backlight: (),
                screen_spi: self.screen_spi,
                display: self.display,
                wifi: self.wifi,
                rtc: self.rtc,
            },
        )
    }
    pub fn screen_spi_peripheral(
        self,
    ) -> (ScreenSpi, Board<Backlight, (), Display, Wifi, RTCUtils>) {
        (
            self.screen_spi,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: (),
                display: self.display,
                wifi: self.wifi,
                rtc: self.rtc,
            },
        )
    }
    pub fn display_peripheral(self) -> (Display, Board<Backlight, ScreenSpi, (), Wifi, RTCUtils>) {
        (
            self.display,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: self.screen_spi,
                display: (),
                wifi: self.wifi,
                rtc: self.rtc,
            },
        )
    }
    pub fn wifi_peripheral(self) -> (Wifi, Board<Backlight, ScreenSpi, Display, (), RTCUtils>) {
        (
            self.wifi,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: self.screen_spi,
                display: self.display,
                wifi: (),
                rtc: self.rtc,
            },
        )
    }

    pub fn rtc_peripheral(self) -> (RTCUtils, Board<Backlight, ScreenSpi, Display, Wifi, ()>) {
        (
            self.rtc,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: self.screen_spi,
                display: self.display,
                wifi: self.wifi,
                rtc: (),
            },
        )
    }
}

impl<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
    Board<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
{
    pub fn backlight<T>(self, p: T) -> Board<T, ScreenSpi, Display, Wifi, RTCUtils> {
        Board {
            screen_backlight: p,
            screen_spi: self.screen_spi,
            display: self.display,
            wifi: self.wifi,
            rtc: self.rtc,
        }
    }

    pub fn screen_spi<T>(self, s: T) -> Board<Backlight, T, Display, Wifi, RTCUtils> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: s,
            display: self.display,
            wifi: self.wifi,
            rtc: self.rtc,
        }
    }
    pub fn display<T>(self, d: T) -> Board<Backlight, ScreenSpi, T, Wifi, RTCUtils> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: self.screen_spi,
            display: d,
            wifi: self.wifi,
            rtc: self.rtc,
        }
    }
    pub fn wifi<T>(self, w: T) -> Board<Backlight, ScreenSpi, Display, T, RTCUtils> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: self.screen_spi,
            display: self.display,
            wifi: w,
            rtc: self.rtc,
        }
    }
    pub fn rtc<T>(self, r: T) -> Board<Backlight, ScreenSpi, Display, Wifi, T> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: self.screen_spi,
            display: self.display,
            wifi: self.wifi,
            rtc: r,
        }
    }
}
