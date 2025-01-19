use hal::gpio::AnyPin;
use hal::timer::Timer;
use hal::{gpio::Output, prelude::*};

pub mod types {
    use display_interface_spi::SPIInterface;
    use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
    use hal::gpio::{AnyPin, Output};
    use hal::ledc::channel::Channel;
    use hal::ledc::LowSpeed;
    use hal::peripherals::SPI2;
    use hal::spi::master::Spi;
    use hal::spi::FullDuplexMode;
    use mipidsi::models::ST7789;
    use mipidsi::Display;

    pub type SPI = SPI2;
    pub type DisplaySPI = Spi<'static, SPI2, FullDuplexMode>;

    pub type LedChannel = Channel<'static, LowSpeed, AnyPin>;
    pub type DisplayImpl<T> = Display<
        SPIInterface<
            ExclusiveDevice<DisplaySPI, Output<'static, AnyPin>, NoDelay>,
            Output<'static, AnyPin>,
        >,
        T,
        Output<'static, AnyPin>,
    >;
}

// https://github.com/SlimeVR/SlimeVR-Rust/blob/main/firmware/src/peripherals/mod.rs
pub struct SpiScreen<SPI> {
    pub dc: Output<'static, AnyPin>,
    pub rst: Output<'static, AnyPin>,
    pub cs_output: Output<'static, AnyPin>,
    pub spi: SPI,
}

pub struct Board<Backlight = (), ScreenSpi = (), Display = ()> {
    pub screen_backlight: Backlight,
    pub screen_spi: ScreenSpi,
    pub display: Display,
    // _lifetime: PhantomData<&'d mut Backlight>,
}

impl Board {
    pub fn new() -> Self {
        Self {
            screen_backlight: (),
            screen_spi: (),
            display: (),
        }
    }
}

/// Type-level destructors for `Board` which turn peripheral type into () to solve partial move.
impl<Backlight, ScreenSpi, Display> Board<Backlight, ScreenSpi, Display> {
    pub fn backlight_peripheral(self) -> (Backlight, Board<(), ScreenSpi, Display>) {
        (
            self.screen_backlight,
            Board {
                screen_backlight: (),
                screen_spi: self.screen_spi,
                display: self.display,
            },
        )
    }
    pub fn screen_spi_peripheral(self) -> (ScreenSpi, Board<Backlight, (), Display>) {
        (
            self.screen_spi,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: (),
                display: self.display,
            },
        )
    }
    pub fn display_peripheral(self) -> (Display, Board<Backlight, ScreenSpi, ()>) {
        (
            self.display,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: self.screen_spi,
                display: (),
            },
        )
    }
}

impl<Backlight, ScreenSpi, Display> Board<Backlight, ScreenSpi, Display> {
    pub fn backlight<T>(self, p: T) -> Board<T, ScreenSpi, Display> {
        Board {
            screen_backlight: p,
            screen_spi: self.screen_spi,
            display: self.display,
        }
    }

    pub fn screen_spi<T>(self, s: T) -> Board<Backlight, T, Display> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: s,
            display: self.display,
        }
    }
    pub fn display<T>(self, d: T) -> Board<Backlight, ScreenSpi, T> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: self.screen_spi,
            display: d,
        }
    }
}
