use core::cell::{Cell, Ref, RefCell, RefMut};

use alloc::{borrow::ToOwned, boxed::Box, rc::Rc};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_graphics::{pixelcolor::raw::RawU16, prelude::Dimensions};
use esp_hal::{gpio::Output, time, timer::systimer::SystemTimer};
use esp_hal_embassy::Executor;
use mipidsi::{
    interface::InterfacePixelFormat,
    models::{Model, ST7789},
};
use slint::{platform::software_renderer::MinimalSoftwareWindow, Window};
use static_cell::StaticCell;
use types::DisplayImpl;

use crate::boards;

pub mod types {
    use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
    use esp_hal::gpio::Output;
    use esp_hal::ledc::channel::Channel;
    pub use esp_hal::ledc::channel::ChannelIFace;
    use esp_hal::spi::master::SpiDmaBus;
    use mipidsi::interface::SpiInterface;

    use esp_hal::ledc::LowSpeed;
    use esp_hal::spi::master::Spi;
    use esp_hal::{Async, Blocking};
    use mipidsi::Display;

    // pub type SPI =  peripherals.SPI2,
    pub type DisplaySPI = SpiDmaBus<'static, Async>;

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

// #[derive(Default)]

#[embassy_executor::task]
async fn refresh_screen(
    window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
    display: DisplayImpl<ST7789>,
) {
    // let display = displayRef;

    let mut buffer_provider = DrawBuffer {
        display: display,
        buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 240],
    };
    if let Some(window) = window.borrow().as_ref().clone() {
        loop {
            let start = time::now();
            slint::platform::update_timers_and_animations();
            // let mut event_count = 0;
            // The hardware keeps a queue of events. We should ideally process all event from the queue before rendering
            // or we will get outdated event in the next frames. But move events are constantly added to the queue
            // so we would block the whole interface, so add an arbitrary threshold
            // while event_count < 15 && touch.data_available().unwrap() {
            //     event_count += 1;
            //     match touch.event() {
            //         // Ignore error because we sometimes get an error at the beginning
            //         Err(_) => (),
            //         Ok(tt21100::Event::Button(..)) => (),
            //         Ok(tt21100::Event::Touch { report: _, touches }) => {
            //             let button = slint::platform::PointerEventButton::Left;
            //             if let Some(event) = touches
            //                 .0
            //                 .map(|record| {
            //                     let position = slint::PhysicalPosition::new(
            //                         ((319. - record.x as f32) * size.width as f32 / 319.) as _,
            //                         (record.y as f32 * size.height as f32 / 239.) as _,
            //                     )
            //                     .to_logical(window.scale_factor());
            //                     match last_touch.replace(position) {
            //                         Some(_) => WindowEvent::PointerMoved { position },
            //                         None => WindowEvent::PointerPressed { position, button },
            //                     }
            //                 })
            //                 .or_else(|| {
            //                     last_touch.take().map(|position| WindowEvent::PointerReleased {
            //                         position,
            //                         button,
            //                     })
            //                 })
            //             {
            //                 let is_pointer_release_event =
            //                     matches!(event, WindowEvent::PointerReleased { .. });

            //                 window.try_dispatch_event(event)?;

            //                 // removes hover state on widgets
            //                 if is_pointer_release_event {
            //                     window.try_dispatch_event(WindowEvent::PointerExited)?;
            //                 }
            //             }
            //         }
            //     }
            // }
            // window.try_dispatch_event(event)
            window.draw_if_needed(|renderer| {
                renderer.render_by_line(&mut buffer_provider);
            });
            let total = time::now() - start;
            log::info!("slint drawing time {}", total);
            if !window.has_active_animations() {
                if let Some(duration) = slint::platform::duration_until_next_timer_update() {
                    let millis = duration.as_millis().try_into().unwrap();
                    log::info!("will sleep for {}ms", millis);
                    Timer::after(Duration::from_millis(millis)).await;
                } else {
                    // https://github.com/slint-ui/slint/discussions/3994
                    // use a stream to await next event, and don't sleep for ever.
                }
            } else {
                log::info!("will sleep for 20ms to let other task.");
                Timer::after(Duration::from_millis(2)).await;
            }
        }
    }
}

#[embassy_executor::task]
async fn embassy_main(
    spawner: Spawner,
    window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
    post_init: Rc<dyn Fn(Spawner, Board<types::LedChannel, (), ()>) -> () + 'static>,
) -> ! {
    log::info!("init embassy");
    let board = boards::init();
    log::info!("init embassy done");
    let (display, board) = board.display_peripheral();

    // let display = self.display.borrow_mut();

    use embedded_graphics::geometry::OriginDimensions;

    let size = display.size();
    let size = slint::PhysicalSize::new(size.width, size.height);

    window.borrow().as_ref().unwrap().set_size(size);

    // let window = singleton!(x, Option<&Rc<MinimalSoftwareWindow>>);

    spawner.spawn(refresh_screen(window, display)).unwrap();
    (post_init)(spawner, board);
    // spawner.spawn(refresh_screen(self.window));
    loop {
        Timer::after(Duration::from_millis(5_000)).await;
    }
}

pub struct EspEmbassyBackend {
    window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
    post_init: Rc<dyn Fn(Spawner, Board<types::LedChannel, (), ()>) -> ()>,
}
impl EspEmbassyBackend {
    pub fn new(
        post_init: impl Fn(Spawner, Board<types::LedChannel, (), ()>) -> () + 'static,
    ) -> Self {
        Self {
            window: RefCell::default(),
            post_init: Rc::new(post_init),
        }
    }
}

impl slint::platform::Platform for EspEmbassyBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = slint::platform::software_renderer::MinimalSoftwareWindow::new(
            slint::platform::software_renderer::RepaintBufferType::ReusedBuffer,
        );
        self.window.replace(Some(window.clone()));
        Ok(window)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_micros(time::now().duration_since_epoch().to_micros())
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        let executor = singleton!(Executor::new(), Executor);

        executor.run(|spawner: Spawner| {
            spawner.must_spawn(embassy_main(
                spawner,
                self.window.clone(),
                self.post_init.to_owned(),
            ));
        })
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        esp_println::println!("{}", arguments);
    }
}

struct DrawBuffer<'a, Display> {
    display: Display,
    buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
}

impl slint::platform::software_renderer::LineBufferProvider
    for &mut DrawBuffer<'_, DisplayImpl<ST7789>>
{
    type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [slint::platform::software_renderer::Rgb565Pixel]),
    ) {
        let buffer = &mut self.buffer[range.clone()];

        render_fn(buffer);

        // We send empty data just to get the device in the right window
        self.display
            .set_pixels(
                range.start as u16,
                line as _,
                range.end as u16,
                line as u16,
                buffer.iter().map(|x| RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}
