#![no_std]
#![no_main]

extern crate alloc;

use embedded_graphics::{
    pixelcolor::{BinaryColor, Rgb565},
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
};

use alloc::format;
// use embedded_graphics_framebuf::FrameBuf;

use board::types::ChannelIFace;
use embassy_time::Duration;

use crate::board::types::LedChannel;
use embassy_executor::Spawner;
use embassy_time::Timer;
use embedded_graphics::mono_font::iso_8859_10::FONT_6X10;
use embedded_graphics::mono_font::iso_8859_15::FONT_7X13_BOLD;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Alignment, Text};
use esp_hal::reset::software_reset;
use esp_hal::time;
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
        log::info!("Panic !! {}", _info.message());
        for _ in 0..10_000_000 {}
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

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_alloc::heap_allocator!(10 * 1024);
    esp_println::logger::init_logger_from_env();
    let board = boards::init();
    let (mut display, board) = board.display_peripheral();
    spawner.spawn(fade_screen(board.screen_backlight)).ok();

    draw_smiley(&mut display).unwrap();
    Timer::after(Duration::from_millis(2000)).await;
    // If looping, don't forget to await something, otherwise the program will just hang
    let start = time::now();
    display.clear(Rgb565::RED).unwrap();
    let total = time::now() - start;
    log::info!("solid drawing time {}", total);
    let mut i = 0;
    let style = MonoTextStyle::new(&FONT_7X13_BOLD, Rgb565::WHITE);

    // let mut data = [Rgb565::WHITE; 240 * 320];

    // let mut fbuf = FrameBuf::new(&mut data, 240, 320);

    loop {
        let text = format!("Hello, World! {}", i);

        let text_area = Text::with_alignment(&text, Point::new(50, 100), style, Alignment::Left);
        let bb = text_area.bounding_box();

        let start = time::now();
        Rectangle::new(bb.top_left, bb.size)
            .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
            .draw(&mut display)
            .unwrap();
        text_area.draw(&mut display).unwrap();
        // display.set_pixels(0, 0, 239, 320, data).unwrap();
        let total = time::now() - start;
        log::info!("text drawing time {}", total);
        Timer::after(Duration::from_millis(100)).await;

        i = i + 1;
    }
}
