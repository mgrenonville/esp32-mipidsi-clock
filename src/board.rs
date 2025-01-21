use esp_hal::gpio::Output;

pub mod types {
    use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
    use esp_hal::gpio::Output;
    use esp_hal::ledc::channel::Channel;
    pub use esp_hal::ledc::channel::ChannelIFace;
    use mipidsi::interface::SpiInterface;

    use esp_hal::ledc::LowSpeed;
    use esp_hal::spi::master::Spi;
    use esp_hal::Blocking;
    use mipidsi::Display;

    // pub type SPI =  peripherals.SPI2,
    pub type DisplaySPI = Spi<'static, Blocking>;

    pub type LedChannel = Channel<'static, LowSpeed>;
    pub type DisplayImpl<T> = Display<
        SpiInterface<
            'static,
            ExclusiveDevice<DisplaySPI, Output<'static>, NoDelay>,
            Output<'static>,
        >,
        T,
        Output<'static>,
    >;
}

// https://github.com/SlimeVR/SlimeVR-Rust/blob/main/firmware/src/peripherals/mod.rs
pub struct SpiScreen<SPI> {
    pub dc: Output<'static>,
    pub rst: Output<'static>,
    pub cs_output: Output<'static>,
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
