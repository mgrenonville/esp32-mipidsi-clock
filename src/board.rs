use hal::gpio::AnyPin;
use hal::timer::Timer;
use hal::{gpio::Output, prelude::*};

pub mod types {
    use hal::gpio::AnyPin;
    use hal::ledc::channel::Channel;
    use hal::ledc::LowSpeed;
    use hal::peripherals::SPI2;
    use hal::spi::master::Spi;
    use hal::spi::FullDuplexMode;

    pub type SPI = SPI2;
    pub type DisplaySPI = Spi<'static, SPI2, FullDuplexMode>;

    pub type LedChannel = Channel<'static, LowSpeed, AnyPin>;
}

// https://github.com/SlimeVR/SlimeVR-Rust/blob/main/firmware/src/peripherals/mod.rs
pub struct SpiScreen<SPI> {
    pub dc: Output<'static, AnyPin>,
    pub rst: Output<'static, AnyPin>,
    pub cs_output: Output<'static, AnyPin>,
    pub spi: SPI,
}

pub struct Board<Backlight = (), Screen = ()> {
    pub screen_backlight: Backlight,
    pub screen: Screen,
    // _lifetime: PhantomData<&'d mut Backlight>,
}

impl Board {
    pub fn new() -> Self {
        Self {
            screen_backlight: (),
            screen: (),
        }
    }
}

/// Type-level destructors for `Board` which turn peripheral type into () to solve partial move.
impl<Backlight, Screen> Board<Backlight, Screen> {
    pub fn backlight_peripheral(self) -> (Backlight, Board<(), Screen>) {
        (
            self.screen_backlight,
            Board {
                screen_backlight: (),
                screen: self.screen,
            },
        )
    }
    pub fn screen_peripheral(self) -> (Screen, Board<Backlight, ()>) {
        (
            self.screen,
            Board {
                screen_backlight: self.screen_backlight,
                screen: (),
            },
        )
    }
}

impl<Backlight, Screen> Board<Backlight, Screen> {
    pub fn backlight<T>(self, p: T) -> Board<T, Screen> {
        Board {
            screen_backlight: p,
            screen: self.screen,
        }
    }

    pub fn screen<T>(self, s: T) -> Board<Backlight, T> {
        Board {
            screen_backlight: self.screen_backlight,
            screen: s,
        }
    }
}
