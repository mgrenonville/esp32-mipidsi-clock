#![no_std]
#![no_main]

use slint::Model;

slint::include_modules!();

extern crate alloc;

use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
    text::renderer::CharacterStyle,
};

use alloc::{boxed::Box, format};
use embedded_graphics_framebuf::FrameBuf;

use board::{types::ChannelIFace, EspEmbassyBackend};
use embassy_time::Duration;

use crate::board::types::LedChannel;
use embassy_executor::Spawner;
use embassy_time::Timer;
use embedded_graphics::mono_font::iso_8859_15::FONT_7X13_BOLD;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::{Alignment, Text};
use esp_hal::{main, reset::software_reset};

use esp_hal::time;
use esp_hal_embassy::Executor;
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
        if let Some(location) = _info.location() {
            log::info!(
                "panic occurred in file '{}' at line {}",
                location.file(),
                location.line(),
            );
        } else {
            log::info!("panic occurred but can't get location information...");
        }
        software_reset();
    }
}

#[embassy_executor::task]
async fn fade_screen(bl: LedChannel) {
    let mut bl_level = 20;

    let mut increase = true;
    loop {
        if bl_level > 99 {
            increase = false;
        } else if bl_level < 20 {
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

#[main]
fn main() -> ! {
    esp_alloc::heap_allocator!(50 * 1024);
    esp_println::logger::init_logger_from_env();

    slint::platform::set_platform(Box::new(EspEmbassyBackend::new()))
        .expect("backend already initialized");
    // spawner.spawn(fade_screen(board.screen_backlight)).ok();
    let main_window = Recipe::new().unwrap();

    let state = main_window.clone_strong();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        core::time::Duration::from_millis(1000),
        move || {
            if state.get_counter() <= 0 {
                state.set_counter(25);
            } else {
                state.set_counter(0);
            }
        },
    );

    main_window.run().unwrap();
    // // draw_smiley(&mut display).unwrap();
    // Timer::after(Duration::from_millis(2000)).await;
    // // If looping, don't forget to await something, otherwise the program will just hang
    // let start = time::now();
    // display.clear(Rgb565::RED).unwrap();
    // let total = time::now() - start;
    // log::info!("solid drawing time {}", total);
    // let mut i = 0;
    // let mut style = MonoTextStyle::new(&FONT_7X13_BOLD, Rgb565::WHITE);
    // style.set_background_color(Option::Some(Rgb565::BLACK));

    // let raw_buf = [Rgb565::BLACK; 320 * 240];

    // loop {
    //     let text = format!("Hello, World! {}", i);

    //     let text_area = Text::with_alignment(&text, Point::new(50, 100), style, Alignment::Left);
    //     // let bb = text_area.bounding_box();

    //     text_area.draw(&mut display).unwrap();
    //     let start = time::now();
    //     // Rectangle::new(bb.top_left, bb.size)
    //     //     .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
    //     //     .draw(&mut display)
    //     //     .unwrap();
    //     display
    //         .fill_solid(&display.bounding_box(), Rgb565::BLACK)
    //         .unwrap();
    //     // display.set_pixels(0, 0, 239, 320, data).unwrap();
    //     let total = time::now() - start;
    //     log::info!("text drawing time {}", total);
    //     Timer::after(Duration::from_millis(1)).await;

    //     i = i + 1;
    // }
    loop {}
}
