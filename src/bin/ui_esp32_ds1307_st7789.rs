#![no_std]
#![no_main]
#![macro_use]

extern crate alloc;

use core::future::IntoFuture;

use alloc::vec;
use alloc::{boxed::Box, rc::Rc};
use chrono::Timelike;
use chrono_tz::Europe::Paris;
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_net::StackResources;
use embassy_net::{Runner, Stack};
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Instant, Ticker, Timer};
use embedded_graphics::prelude::Point;
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal_bus::spi::ExclusiveDevice;
use esp32_mipidsi_clock::controller::WallClock;
use esp32_mipidsi_clock::ntp::{await_now, now, NtpClient};
use esp32_mipidsi_clock::wifi::EspEmbassyWifiController;
use esp_hal::{
    clock::CpuClock,
    delay::Delay,
    dma::{DmaRxBuf, DmaTxBuf},
    i2c::master::I2c,
    ledc::{
        channel::{self, config::PinConfig, ChannelIFace},
        timer::{self, TimerIFace},
        LSGlobalClkSource, Ledc, LowSpeed,
    },
    rng::Rng,
    rtc_cntl::Rtc,
    time::RateExtU32,
    timer::timg::TimerGroup,
    tsens::TemperatureSensor,
};
use esp_hal::{
    dma_buffers,
    gpio::{Level, Output},
    spi::{
        master::{Config, Spi},
        Mode,
    },
    time,
};

use esp_backtrace as _;

use ds323x::{DateTimeAccess, Ds323x, NaiveDate};
use esp32_mipidsi_clock::{
    board::{types::LedChannel, Board},
    boards::DrawBuffer,
    controller::Controller,
    slintplatform::EspEmbassyBackend,
};
use esp32_mipidsi_clock::{
    board::{
        types::{DisplayImpl, RTCUtils},
        RtcRelated,
    },
    controller::{self, Action},
};
use esp_wifi::{
    wifi::{WifiDevice, WifiStaDevice},
    EspWifiController,
};
use log::{info, log};
use mipidsi::{
    interface::SpiInterface,
    models::GC9A01,
    // models::ST7789,
    options::{ColorInversion, TearingEffect},
    Builder,
};
use slint::{
    platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    ComponentHandle,
};

use slint_generated::{Globals, Recipe, TimeOfDay};

macro_rules! singleton {
    ($val:expr, $T:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$T> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

pub const DISPLAY_WIDTH: usize = 240;
pub const DISPLAY_HEIGHT: usize = 240;

const SLINT_TARGET_FPS: u64 = 25;
const SLINT_FRAME_DURATION_MS: u64 = 1000 / SLINT_TARGET_FPS;

#[esp_hal_embassy::main]
async fn main(spawner: Spawner) {
    esp_alloc::heap_allocator!(100 * 1024);
    esp_println::logger::init_logger_from_env();

    let mut config = esp_hal::Config::default();
    config.cpu_clock = CpuClock::_160MHz;
    let peripherals = esp_hal::init(config);

    // log::info!("running at {}", peripherals.);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    esp_hal_embassy::init(timg0.timer0);

    let rtc = Rtc::new(peripherals.LPWR);

    let tsen =
        TemperatureSensor::new(peripherals.TSENS, esp_hal::tsens::Config::default()).unwrap();
    tsen.power_up();

    let mut ledc = Ledc::new(peripherals.LEDC);
    ledc.set_global_slow_clock(LSGlobalClkSource::APBClk);
    let lstimer0 = singleton!(
        ledc.timer::<LowSpeed>(timer::Number::Timer1),
        timer::Timer<LowSpeed>
    );
    lstimer0
        .configure(timer::config::Config {
            duty: timer::config::Duty::Duty5Bit,
            clock_source: timer::LSClockSource::APBClk,
            frequency: 24u32.kHz(),
        })
        .unwrap();
    let led = Output::new(peripherals.GPIO5, Level::Low);

    let mut channel0 = ledc.channel(channel::Number::Channel0, led);
    channel0
        .configure(channel::config::Config {
            timer: lstimer0,
            duty_pct: 10,

            pin_config: PinConfig::PushPull,
        })
        .unwrap();

    let dc = Output::new(peripherals.GPIO15, Level::Low);
    let sck = peripherals.GPIO18;
    let mosi = peripherals.GPIO19;
    let cs = peripherals.GPIO4;

    // Define the reset pin as digital outputs and make it high
    let mut rst = Output::new(peripherals.GPIO3, Level::Low);
    rst.set_high();

    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(1, 240);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    // Define the SPI pins and create the SPI interface
    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(80u32.MHz())
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sck)
    .with_mosi(mosi)
    .with_dma(peripherals.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    let cs_output = Output::new(cs, Level::High);

    let spi_device = ExclusiveDevice::new_no_delay(spi, cs_output).unwrap();

    // Define the display interface with no chip select
    let buffer = singleton!([0_u8; 240], [u8; 240]);
    let di = SpiInterface::new(spi_device, dc, buffer);
    // Define the display from the display interface and initialize it
    let mut delay = Delay::new();

    let mut display = Builder::new(GC9A01, di)
        .reset_pin(rst)
        .display_size(240, 240)
        .color_order(mipidsi::options::ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        // .orientation(Orientation::new().rotate(Rotation::Deg180))
        .init(&mut delay)
        .unwrap();

    // // Make the display all
    // match display.set_tearing_effect(TearingEffect::Vertical) {
    //     Ok(_) => log::info!("set_tearing_effect successful"),
    //     Err(e) => log::info!("set_tearing_effect failed"),
    // };
    display.clear(Rgb565::WHITE).unwrap();
    // display.clear(Rgb565::RED).unwrap();

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let mut rng = Rng::new(peripherals.RNG);

    let init = &*singleton!(
        esp_wifi::init(timg1.timer0, rng.clone(), peripherals.RADIO_CLK).unwrap(),
        EspWifiController<'static>
    );

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller): (
        esp_wifi::wifi::WifiDevice<'_, esp_wifi::wifi::WifiStaDevice>,
        esp_wifi::wifi::WifiController<'_>,
    ) = esp_wifi::wifi::new_with_mode(&init, wifi, esp_wifi::wifi::WifiStaDevice).unwrap();

    let config = embassy_net::Config::dhcpv4(Default::default());

    let seed = (rng.random() as u64) << 32 | rng.random() as u64;

    // Init network stack
    let (stack, runner): (
        embassy_net::Stack<'_>,
        embassy_net::Runner<'_, esp_wifi::wifi::WifiDevice<'_, esp_wifi::wifi::WifiStaDevice>>,
    ) = embassy_net::new(
        wifi_interface,
        config,
        singleton!(StackResources::<3>::new(), StackResources<3>),
        seed,
    );

    let i2c = I2c::new(peripherals.I2C0, esp_hal::i2c::master::Config::default())
        .ok()
        .unwrap()
        .with_scl(peripherals.GPIO6)
        .with_sda(peripherals.GPIO7);

    // let mut ds1307 = Ds1307::new(i2c);
    let mut ds3231: Ds323x<
        ds323x::interface::I2cInterface<I2c<'_, esp_hal::Blocking>>,
        ds323x::ic::DS3231,
    > = Ds323x::new_ds3231(i2c);
    // ds1307.set_running().ok();

    // let datetime = ds1307.datetime().unwrap();
    // log::info!("DS1307: {}", ds1307.running().ok().unwrap());
    let board = Board::new().backlight(channel0).rtc(RtcRelated {
        ds1307: Mutex::new(ds3231),
        rtc,
        temperature_sensor: tsen,
    });

    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(slint::PhysicalSize::new(
        DISPLAY_WIDTH as u32,
        DISPLAY_HEIGHT as u32,
    ));
    let backend = Box::new(EspEmbassyBackend::new(window.clone()));

    slint::platform::set_platform(backend).expect("backend already initialized");
    log::info!("slint gui setup complete");

    // TASK: run the gui render loop
    spawner.spawn(render_loop(window, display)).unwrap();
    let (bl, board) = board.backlight_peripheral();
    let (rtc, board) = board.rtc_peripheral();
    let rtc_rc = Rc::new(rtc);

    let _ = spawner
        .spawn(run_wifi_controller(EspEmbassyWifiController::new(
            controller,
        )))
        .ok();
    let _ = spawner.spawn(net_task(runner)).ok();

    let ntp_client = NtpClient::new(stack);

    let _ = spawner.spawn(fade_screen(bl, rtc_rc.clone())).unwrap();
    let _ = spawner.spawn(run_ntp_client(ntp_client));
    let _ = spawner.spawn(update_rtc_with_ntp(rtc_rc.clone()));
    let _ = spawner.spawn(wifi_status_task(stack));

    let _ = spawner.spawn(update_timer(rtc_rc.clone()));

    let recipe = Recipe::new().unwrap();

    recipe.show().expect("unable to show main window");

    // run the controller event loop
    let mut controller = Controller::new(&recipe, board, rtc_rc.clone());
    controller.run().await;
}

#[embassy_executor::task]
async fn run_wifi_controller(mut wifi_controller: EspEmbassyWifiController<'static>) {
    wifi_controller.connection().await;
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}

#[embassy_executor::task]
async fn render_loop(window: Rc<MinimalSoftwareWindow>, display: DisplayImpl<GC9A01>) {
    // let display = displayRef;

    let mut buffer_provider = DrawBuffer {
        display: display,
        buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 240],
    };
    loop {
        log::trace!("{} - slint drawing start!", Instant::now().as_millis());

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
        let dirty = window.draw_if_needed(|renderer| {
            renderer.render_by_line(&mut buffer_provider);
        });
        let total = time::now() - start;
        log::trace!(
            "{} - slint drawing time {}, active anims: {}, dirty: {}",
            Instant::now().as_millis(),
            total,
            window.has_active_animations(),
            dirty
        );
        if !window.has_active_animations() {
            if let Some(duration) = slint::platform::duration_until_next_timer_update() {
                let millis = duration.as_millis().try_into().unwrap();
                log::trace!("will sleep for {}ms", millis);
                select(
                    controller::refresh_screen(),
                    Timer::after(Duration::from_millis(millis)),
                )
                .await;
            } else {
                // https://github.com/slint-ui/slint/discussions/3994
                log::trace!(
                    "{} - will sleep until refresh_screen asked",
                    Instant::now().as_millis()
                );
                let _ = controller::refresh_screen().await;
                // Timer::after(Duration::from_millis(10)).await;
                log::trace!("{} - refresh_screen asked", Instant::now().as_millis());
            }
        } else {
            let pause_for_target_fps = SLINT_FRAME_DURATION_MS as i32 - total.to_millis() as i32;

            if (pause_for_target_fps > 0) {
                log::trace!(
                    "will sleep for {}ms to achieve {}fps",
                    pause_for_target_fps,
                    SLINT_TARGET_FPS
                );
                Timer::after(Duration::from_millis(pause_for_target_fps as u64)).await;
            } else {
                log::trace!("will sleep for 1ms, late on FPS");
                Timer::after(Duration::from_millis(1)).await;
            }
        }
    }
}

/** A task to prove that we can do other things that render_loops */
#[embassy_executor::task]
async fn fade_screen(bl: LedChannel, rtc: Rc<RTCUtils>) {
    loop {
        let d = rtc.get_date_time().await.with_timezone(&Paris);
        let mut bl_level = 5;
        if (d.hour() > 8 && d.hour() < 20) {
            bl_level = 100;
        } else if (d.hour() >= 20 && d.hour() < 21) {
            bl_level = 30;
        }
        bl.set_duty(bl_level).unwrap();
        log::trace!("Setting backlight to {}", bl_level);
        Timer::after_secs(10).await;
        // Timer::after_millis(10).await;
        // bl.set_duty(bl_level).unwrap();
        // if increase {
        //     bl_level = bl_level + 1;
        // } else {
        //     bl_level = bl_level - 1;
        // }
    }
}

#[embassy_executor::task]
async fn print_stats() {
    loop {
        let stats = esp_alloc::HEAP.stats();
        // HeapStats implements the Display and defmt::Format traits, so you can pretty-print the heap stats.
        log::info!("{}", stats);
        Timer::after_millis(1000).await;
    }
}

#[embassy_executor::task]
async fn wifi_status_task(stack: Stack<'static>) {
    loop {
        if (stack.is_link_up()) {
            if (stack.is_config_up()) {
                controller::send_action(Action::WifiStateUpdate(slint_generated::WifiState::OK));
            } else {
                controller::send_action(Action::WifiStateUpdate(
                    slint_generated::WifiState::LINKUP,
                ));
            }
        } else {
            controller::send_action(Action::WifiStateUpdate(
                slint_generated::WifiState::STARTING,
            ));
        }

        // refresh_signal.signal(());
        if (!stack.is_config_up()) {
            Timer::after(Duration::from_millis(50)).await;
        } else {
            Timer::after(Duration::from_secs(10)).await;
        }
    }
}
#[embassy_executor::task]

async fn update_rtc_with_ntp(rtc: Rc<RTCUtils>) {
    loop {
        let now = await_now().await;
        info!("Update time ! {}", now);

        rtc.set_date_time(now.to_utc()).await;
        Timer::after(Duration::from_secs(10)).await;
    }
}

#[embassy_executor::task]
async fn run_ntp_client(ntp_client: NtpClient<'static>) {
    ntp_client.run().await;
}

#[embassy_executor::task]
async fn update_timer(rtc: Rc<RTCUtils>) {
    let mut visible = true;
    let mut last_value = 0;
    let mut ticker = Ticker::every(Duration::from_millis(1000));
    loop {
        let current_time = rtc.get_date_time().await.with_timezone(&Paris);

        let actual = current_time.second() % 10;
        if (actual != last_value) {
            visible = !visible;
        }
        last_value = actual;
        let mut tod = TimeOfDay::DAY;
        let mut point = Point { x: 125, y: 188 };
        let mut env = slint_generated::MonsterEnv::OUTSIDE;
        if (current_time.hour() >= 20 || current_time.hour() < 8) {
            tod = TimeOfDay::NIGHT;
            point = Point { x: 195, y: 143 };
            env = slint_generated::MonsterEnv::HOUSE;
        }
        controller::send_action(Action::MultipleActions(vec![
            Action::ShowMonster(visible, point, env),
            Action::UpdateTime(current_time),
            Action::TimeOfDayUpdate(tod),
        ]));

        log::debug!(
            "Setting visible monster: {} (actual: {}, last_value{}, current_time: {})",
            visible,
            actual,
            last_value,
            current_time
        );
        // trigger refresh
        // refresh_signal.signal(());

        // Double trigger

        ticker.next().await;
        // Timer::after_millis(10).await;
    }
}
