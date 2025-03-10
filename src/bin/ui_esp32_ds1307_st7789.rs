#![no_std]
#![no_main]
#![macro_use]

extern crate alloc;

use core::net::{IpAddr, SocketAddr};

use chrono::{DateTime, Timelike};
use chrono_tz::Europe::Paris;
use embassy_futures::select::select;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    Runner, Stack,
};
use ds1307::DateTimeAccess;
use alloc::{boxed::Box, rc::Rc};
use embassy_executor::Spawner;
use embassy_net::StackResources;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex, signal::Signal};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::{draw_target::DrawTarget, pixelcolor::Rgb565, prelude::RgbColor};
use embedded_hal_bus::spi::ExclusiveDevice;
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

use ds1307::Ds1307;
use esp32_mipidsi_clock::{board::{types::LedChannel, Board}, boards::DrawBuffer, controller::Controller, slintplatform::EspEmbassyBackend};
use esp32_mipidsi_clock::{
    board::{
        types::{DisplayImpl, RTCUtils},
        RtcRelated, Wifi,
    },
    controller::{self, Action},
};
use esp_wifi::{
    wifi::{ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice, WifiState},
    EspWifiController,
};
use mipidsi::{
    interface::SpiInterface,
    models::ST7789,
    options::{ColorInversion, TearingEffect},
    Builder,
};
use slint::{
    platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
    ComponentHandle,
};

use slint_generated::Recipe;
use smoltcp::wire::DnsQueryType;
use sntpc::{get_time, NtpContext, NtpTimestampGenerator};

macro_rules! singleton {
    ($val:expr, $T:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$T> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

pub const DISPLAY_WIDTH: usize = 240;
pub const DISPLAY_HEIGHT: usize = 240;

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const NTP_SERVER: &str = "pool.ntp.org";

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
    let miso = peripherals.GPIO22;
    let mosi = peripherals.GPIO19;
    let cs = peripherals.GPIO4;

    // Define the reset pin as digital outputs and make it high
    let mut rst = Output::new(peripherals.GPIO2, Level::Low);
    rst.set_high();

    let (rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = dma_buffers!(2400, 2400);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, rx_buffer).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, tx_buffer).unwrap();

    // Define the SPI pins and create the SPI interface
    let spi = Spi::new(
        peripherals.SPI2,
        Config::default()
            .with_frequency(60u32.MHz())
            .with_mode(Mode::_0),
    )
    .unwrap()
    .with_sck(sck)
    .with_mosi(mosi)
    .with_miso(miso)
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

    let mut display = Builder::new(ST7789, di)
        .reset_pin(rst)
        .display_size(240, 240)
        .color_order(mipidsi::options::ColorOrder::Rgb)
        .invert_colors(ColorInversion::Inverted)
        // .orientation(Orientation::new().rotate(Rotation::Deg180))
        .init(&mut delay)
        .unwrap();

    // Make the display all
    match display.set_tearing_effect(TearingEffect::Vertical) {
        Ok(_) => log::info!("set_tearing_effect successful"),
        Err(e) => log::info!("set_tearing_effect failed"),
    };
    display.clear(Rgb565::BLACK).unwrap();

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

    let mut ds1307 = Ds1307::new(i2c);
    ds1307.set_running().ok();

    // let datetime = ds1307.datetime().unwrap();
    log::info!("DS1307: {}", ds1307.running().ok().unwrap());
    let board = Board::new()
        .backlight(channel0)
        .wifi(Wifi {
            stack,
            runner,
            controller,
        })
        .rtc(RtcRelated {
            ds1307: Mutex::new(ds1307),
            rtc,
            temperature_sensor: tsen,
        });

    let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
    window.set_size(slint::PhysicalSize::new(
        DISPLAY_WIDTH as u32,
        DISPLAY_HEIGHT as u32,
    ));
    let backend = Box::new(EspEmbassyBackend::new(
        // &inner_main,
        window.clone(),
    ));

    slint::platform::set_platform(backend).expect("backend already initialized");
    log::info!("slint gui setup complete");

    // TASK: run the gui render loop
    spawner
        .spawn(render_loop(window, display))
        .unwrap();
    let (bl, board) = board.backlight_peripheral();
    let (wifi, board) = board.wifi_peripheral();
    let (rtc, board) = board.rtc_peripheral();
    let rtc_rc = Rc::new(rtc);
    let _ = spawner.spawn(fade_screen(bl)).unwrap();

    let _ = spawner.spawn(connection(wifi.controller)).ok();
    let _ = spawner.spawn(net_task(wifi.runner)).ok();

    let _ = spawner.spawn(ntp_task(wifi.stack, rtc_rc.clone()));
    let _ = spawner.spawn(wifi_status_task(
        wifi.stack,
    ));
    let _ = spawner.spawn(update_timer(rtc_rc.clone()));

    let recipe = Recipe::new().unwrap();
    recipe.show().expect("unable to show main window");
    // recipe.run();
    
    // run the controller event loop
    let mut controller = Controller::new(&recipe, board);
    controller.run().await;
}

#[embassy_executor::task]
async fn render_loop(
    window: Rc<MinimalSoftwareWindow>,
    display: DisplayImpl<ST7789>,
) {
    // let display = displayRef;

    let mut buffer_provider = DrawBuffer {
        display: display,
        buffer: &mut [slint::platform::software_renderer::Rgb565Pixel(0); 240],
    };
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
        log::trace!("slint drawing time {}", total);
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
                controller::refresh_screen().await;
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
async fn fade_screen(bl: LedChannel) {
    let mut bl_level = 20;

    let mut increase = true;
    loop {
        if bl_level > 99 {
            increase = false;
        } else if bl_level < 50 {
            increase = true;
        }
        log::trace!("Setting backlight to {}", bl_level);

        Timer::after_millis(10).await;
        bl.set_duty(bl_level).unwrap();
        if increase {
            bl_level = bl_level + 1;
        } else {
            bl_level = bl_level - 1;
        }
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
async fn connection(mut controller: WifiController<'static>) {
    log::info!("start connection task");
    log::info!("Device capabilities: {:?}", controller.capabilities());
    loop {
        match esp_wifi::wifi::wifi_state() {
            WifiState::StaConnected => {
                // wait until we're no longer connected
                controller.wait_for_event(WifiEvent::StaDisconnected).await;
                Timer::after(Duration::from_millis(5000)).await
            }
            _ => {}
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            log::info!("Starting wifi");
            controller.start_async().await.unwrap();
            log::info!("Wifi started!");
        }
        log::info!("About to connect to {} with {}...", SSID, PASSWORD);

        match controller.connect_async().await {
            Ok(_) => log::info!("Wifi connected!"),
            Err(e) => {
                log::info!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
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
async fn ntp_task(stack: Stack<'static>, rtc: Rc<RTCUtils>) {
    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    log::info!("Waiting to get IP address...");

    stack.wait_config_up().await;

    loop {
        if let Some(config) = stack.config_v4() {
            log::info!("Got IP: {}", config.address);
            break;
        }
        log::info!(".");
        Timer::after(Duration::from_millis(500)).await;
    }

    let mut udp_rx_meta = [PacketMetadata::EMPTY; 16];
    let mut udp_rx_buffer = [0; 1024];
    let mut udp_tx_meta = [PacketMetadata::EMPTY; 16];
    let mut udp_tx_buffer = [0; 1024];

    let mut socket = UdpSocket::new(
        stack,
        &mut udp_rx_meta,
        &mut udp_rx_buffer,
        &mut udp_tx_meta,
        &mut udp_tx_buffer,
    );

    // socket.set_timeout(Some(embassy_time::Duration::from_secs(10)));

    socket.bind(123).unwrap();

    let context = NtpContext::new(Timestamp::new(&rtc));

    let ntp_addrs = stack
        .dns_query(NTP_SERVER, DnsQueryType::A)
        .await
        .expect("Failed to resolve DNS");
    if ntp_addrs.is_empty() {
        log::error!("Failed to resolve DNS");
    }
    let mut start = DateTime::from_timestamp_nanos(0);
    let mut now = DateTime::from_timestamp_micros(Instant::now().as_micros() as i64).unwrap();
    let mut first = true;
    loop {
        let addr: IpAddr = ntp_addrs[0].into();

        let result = get_time(SocketAddr::from((addr, 123)), &socket, context).await;
        // let result = sntp_send_request(SocketAddr::from((addr, 123)), &socket, context).await.ok().unwrap();
        // let response = sntp_process_response(SocketAddr::from((addr, 123)), &socket, context, result).await;
        match result {
            Ok(time) => {
                let datetime = DateTime::from_timestamp(
                    time.sec().into(),
                    (time.sec_fraction() as u64 * 1_000_000_000 / 4_294_967_296) as u32,
                )
                .unwrap();
                if (first) {
                    start = datetime;
                    now =
                        DateTime::from_timestamp_micros(Instant::now().as_micros() as i64).unwrap();
                    rtc.rtc.set_current_time(datetime.naive_local());
                    rtc.ds1307
                        .lock()
                        .await
                        .set_datetime(&datetime.naive_local())
                        .ok();
                    first = false;
                }
                let delta = rtc.rtc.current_time().and_utc() - start;
                let delta_main_clock =
                    DateTime::from_timestamp_micros(Instant::now().as_micros() as i64).unwrap()
                        - now;
                let delta_ntp = datetime - start;
                log::info!(
                    "Time: {:?} @ {}C, offset: {}, roundtrip: {}",
                    datetime,
                    rtc.temperature_sensor.get_temperature().to_celsius(),
                    time.offset(),
                    time.roundtrip()
                );
                log::info!(
                    "Elapsed rtc: {}us, cpu: {}us, ntp: {}us",
                    delta,
                    delta_main_clock,
                    delta_ntp
                );
                log::info!(
                    "Deltas rtc/ntp: {}, cpu/ntp: {}",
                    delta_ntp - delta,
                    delta_ntp - delta_main_clock
                );
            }
            Err(e) => {
                log::error!("Error getting time: {:?}", e);
            }
        }

        Timer::after(Duration::from_secs(15 * 60)).await; // Every 15 minutes
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}

#[derive(Copy, Clone)]
struct Timestamp<'a> {
    duration: Duration,
    rtc: &'a RTCUtils,
}
impl<'a> Timestamp<'a> {
    fn new(rtc: &'a RTCUtils) -> Timestamp<'a> {
        Timestamp {
            duration: Duration::default(),
            rtc,
        }
    }
}

impl<'a> NtpTimestampGenerator for Timestamp<'a> {
    fn init(&mut self) {
        self.duration = Duration::from_millis(
            self.rtc
                .rtc
                .current_time()
                .and_utc()
                .timestamp_millis()
                .try_into()
                .unwrap_or(0),
        );
        log::info!(
            "duration: {}ms, time: {}",
            self.duration.as_millis(),
            self.rtc.rtc.current_time().and_utc()
        );
    }

    fn timestamp_sec(&self) -> u64 {
        self.duration.as_secs()
    }

    fn timestamp_subsec_micros(&self) -> u32 {
        (self.duration.as_micros() - self.duration.as_secs() * 1000000)
            .try_into()
            .unwrap()
    }
}

#[embassy_executor::task]
async fn update_timer(rtc: Rc<RTCUtils>) {
    let mut visible = true;
    let mut last_value = 0;
    loop {
        let current_time = rtc
            .ds1307
            .lock()
            .await
            .datetime()
            .map(|m| m.and_utc())
            .unwrap_or(DateTime::from_timestamp_nanos(0))
            .with_timezone(&Paris);
        controller::send_action(Action::UpdateTime(current_time));
        
        let actual = current_time.second() % 10;
        if (actual != last_value) {
            visible = !visible;
        }
        last_value = actual;
        controller::send_action(Action::ShowMonster(visible));
        log::debug!(
            "Setting visible monster: {} (actual: {}, last_value{}, current_time: {})",
            visible,
            actual,
            last_value,
            current_time
        );
        // trigger refresh
        // refresh_signal.signal(());
        Timer::after_millis(100).await;
    }
}
