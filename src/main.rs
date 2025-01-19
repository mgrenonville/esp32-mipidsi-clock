#![no_std]
#![no_main]

extern crate alloc;

use core::mem::MaybeUninit;

use display_interface_spi::SPIInterface;
use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
};
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
use hal::delay::Delay;
use hal::gpio::Output;
use hal::peripheral::Peripheral;
use hal::reset::software_reset;
use hal::{gpio::AnyPin, prelude::*};
use mipidsi::{Builder, Display};
// Provides the Display builder
use mipidsi::models::ST7789;
use mipidsi::options::{ColorInversion, Orientation, Rotation};

use crate::board::types::DisplaySPI;
use crate::board::{types, SpiScreen};

mod board;
mod boards;

// Provides the parallel port and display interface builders

fn draw_smiley<T: DrawTarget<Color = Rgb565>>(display: &mut T) -> Result<(), T::Error> {
    // Draw the left eye as a circle located at (50, 100), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 100), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw the right eye as a circle located at (50, 200), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 200), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw an upside down red triangle to represent a smiling mouth
    Triangle::new(
        Point::new(130, 140),
        Point::new(130, 200),
        Point::new(160, 170),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
    .draw(display)?;

    // Cover the top part of the mouth with a black triangle so it looks closed instead of open
    Triangle::new(
        Point::new(130, 150),
        Point::new(130, 190),
        Point::new(150, 170),
    )
    .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
    .draw(display)?;

    Ok(())
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        software_reset();
    }
}

#[entry]
fn main() -> ! {
    let delay = Delay::new();
    esp_alloc::heap_allocator!(72 * 1024);

    esp_println::logger::init_logger_from_env();

    let mut board = boards::init();

    // log::info!("Hello world!");
    let (mut display, board) = board.display_peripheral();

    draw_smiley(&mut display).unwrap();

    let mut bl_level = 1;
    let mut increase = true;
    loop {
        if (bl_level > 99) {
            increase = false;
        } else if (bl_level < 1) {
            increase = true;
        }
        // log::info!("Hello world!");
        // log::info!("Setting backlight to {}", bl_level);
        //
        delay.delay(5.millis());
        board.screen_backlight.set_duty(bl_level).unwrap();
        if (increase) {
            bl_level = bl_level + 1;
        } else {
            bl_level = bl_level - 1;
        }
    }
}
