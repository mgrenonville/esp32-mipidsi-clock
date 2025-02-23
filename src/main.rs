#![no_std]
#![no_main]

slint::include_modules!();

extern crate alloc;

use alloc::{boxed::Box, rc::Rc};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;

use esp_hal::{main, reset::software_reset};
use slintplatform::EspEmbassyBackend;

mod board;
mod boards;
mod dmaspi;
mod slintplatform;

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

#[main]
fn main() -> ! {
    esp_alloc::heap_allocator!(100 * 1024);
    esp_println::logger::init_logger_from_env();
    let MAIN_WINDOW_REF = singleton!( Signal::<CriticalSectionRawMutex, Rc<Recipe>>::new(), Signal<CriticalSectionRawMutex, Rc<Recipe>>);

    slint::platform::set_platform(Box::new(EspEmbassyBackend::new(
        // &inner_main,
        MAIN_WINDOW_REF,
    )))
    .expect("backend already initialized");
    // spawner.spawn(fade_screen(board.screen_backlight)).ok();
    let main_window = Recipe::new().unwrap();

    let state = main_window.clone_strong();

    // Allow sharing main_window to embassy code
    MAIN_WINDOW_REF.signal(Rc::new(main_window.clone_strong()));

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        core::time::Duration::from_millis(10000),
        move || {
            if state.get_counter() <= 0 {
                state.set_counter(25);
            } else {
                state.set_counter(0);
            }
        },
    );

    // slint::invoke_from_event_loop(func);
    // https://docs.rs/slint/latest/slint/fn.invoke_from_event_loop.html
    // let weak_main = main_window.as_weak();
    // Idea is to create an InterruptExecutor, and get state from outside, then call a weak ref to component to invoke_from_event_loop
    // https://github.com/slint-ui/slint/discussions/3994 also use waker to sleep correclty in event loop

    main_window.run().unwrap();

    loop {}
}
