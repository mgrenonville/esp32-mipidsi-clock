#![no_std]
#![no_main]

extern crate alloc;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
};

use board::types::ChannelIFace;
use embassy_time::Duration;

use crate::board::types::LedChannel;
use embassy_executor::Spawner;
use embassy_time::Timer;
use esp_hal::reset::software_reset;
use esp_println::println;

mod board;
mod boards;
mod dmaspi;

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

#[embassy_executor::task]
async fn fade_screen(bl: LedChannel) {
    let mut bl_level = 1;

    let mut increase = true;
    loop {
        if bl_level > 99 {
            increase = false;
        } else if bl_level < 1 {
            increase = true;
        }
        esp_println::println!("Setting backlight to {}", bl_level);

        Timer::after_millis(50).await;
        bl.set_duty(bl_level).unwrap();
        if increase {
            bl_level = bl_level + 1;
        } else {
            bl_level = bl_level - 1;
        }
    }
}

#[embassy_executor::task]
async fn run() {
    loop {
        esp_println::println!("Hello world from embassy using esp-hal-async!");
        Timer::after(Duration::from_millis(1_000)).await;
    }
}

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_alloc::heap_allocator!(72 * 1024);
    esp_println::logger::init_logger_from_env();
    let board = boards::init();
    let (mut display, board) = board.display_peripheral();
    spawner.spawn(fade_screen(board.screen_backlight)).ok();

    draw_smiley(&mut display).unwrap();
    // If looping, don't forget to await something, otherwise the program will just hang
    // loop {
    //     Timer::after(Duration::from_millis(5_000)).await;
    // }
}
