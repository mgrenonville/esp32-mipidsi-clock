use core::{
    cell::RefCell,
    net::{IpAddr, SocketAddr},
};

use alloc::{format, rc::Rc};
use chrono::{DateTime, TimeDelta, Timelike};
use chrono_tz::Europe::Paris;
use ds1307::DateTimeAccess;
use ds1307::Ds1307;
use embassy_executor::Spawner;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    Runner, Stack,
};
use embassy_sync::{blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex}, mutex::Mutex, signal::Signal};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::pixelcolor::raw::RawU16;
use esp_hal::{
    gpio::Output, i2c::master::I2c, ledc::channel::ChannelIFace, rtc_cntl::Rtc, time,
    tsens::TemperatureSensor,
};
use esp_hal_embassy::Executor;
use esp_wifi::wifi::{
    ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
    WifiState,
};
use mipidsi::models::ST7789;
use slint::{platform::software_renderer::MinimalSoftwareWindow, SharedString, ToSharedString};
use smoltcp::wire::DnsQueryType;
use sntpc::{get_time, NtpContext, NtpTimestampGenerator};

use types::{DisplayImpl, LedChannel, RTCUtils};

use crate::{boards, Recipe};

pub mod types {
    use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
    use esp_hal::gpio::Output;
    use esp_hal::ledc::channel::Channel;
    pub use esp_hal::ledc::channel::ChannelIFace;
    use esp_hal::rtc_cntl::Rtc;
    use esp_hal::spi::master::SpiDmaBus;
    use mipidsi::interface::SpiInterface;

    use esp_hal::ledc::LowSpeed;
    use esp_hal::Async;
    use mipidsi::Display;

    use super::RtcRelated;

    // pub type SPI =  peripherals.SPI2,
    pub type DisplaySPI = SpiDmaBus<'static, Async>;

    pub type RTCUtils = RtcRelated;
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
#[macro_export]
macro_rules! singleton {
    ($val:expr, $T:ty) => {{
        static STATIC_CELL: ::static_cell::StaticCell<$T> = ::static_cell::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const NTP_SERVER: &str = "pool.ntp.org";

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

// https://github.com/SlimeVR/SlimeVR-Rust/blob/main/firmware/src/peripherals/mod.rs
pub struct SpiScreen<SPI> {
    pub dc: Output<'static>,
    pub rst: Output<'static>,
    pub cs_output: Output<'static>,
    pub spi: SPI,
}

pub struct RtcRelated {
    pub ds1307: Mutex<NoopRawMutex, Ds1307<I2c<'static, esp_hal::Blocking>>>,
    pub rtc: Rtc<'static>,
    pub temperature_sensor: TemperatureSensor<'static>,
}

pub struct Wifi {
    pub stack: embassy_net::Stack<'static>,
    pub runner: embassy_net::Runner<
        'static,
        esp_wifi::wifi::WifiDevice<'static, esp_wifi::wifi::WifiStaDevice>,
    >,
    pub controller: esp_wifi::wifi::WifiController<'static>,
}

pub struct Board<Backlight = (), ScreenSpi = (), Display = (), Wifi = (), RTCUtils = ()> {
    pub screen_backlight: Backlight,
    pub screen_spi: ScreenSpi,
    pub display: Display,
    pub wifi: Wifi,
    pub rtc: RTCUtils,
    // _lifetime: PhantomData<&'d mut Backlight>,
}

impl Board {
    pub fn new() -> Self {
        Self {
            screen_backlight: (),
            screen_spi: (),
            display: (),
            wifi: (),
            rtc: (),
        }
    }
}

/// Type-level destructors for `Board` which turn peripheral type into () to solve partial move.
impl<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
    Board<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
{
    pub fn backlight_peripheral(
        self,
    ) -> (Backlight, Board<(), ScreenSpi, Display, Wifi, RTCUtils>) {
        (
            self.screen_backlight,
            Board {
                screen_backlight: (),
                screen_spi: self.screen_spi,
                display: self.display,
                wifi: self.wifi,
                rtc: self.rtc,
            },
        )
    }
    pub fn screen_spi_peripheral(
        self,
    ) -> (ScreenSpi, Board<Backlight, (), Display, Wifi, RTCUtils>) {
        (
            self.screen_spi,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: (),
                display: self.display,
                wifi: self.wifi,
                rtc: self.rtc,
            },
        )
    }
    pub fn display_peripheral(self) -> (Display, Board<Backlight, ScreenSpi, (), Wifi, RTCUtils>) {
        (
            self.display,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: self.screen_spi,
                display: (),
                wifi: self.wifi,
                rtc: self.rtc,
            },
        )
    }
    pub fn wifi_peripheral(self) -> (Wifi, Board<Backlight, ScreenSpi, Display, (), RTCUtils>) {
        (
            self.wifi,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: self.screen_spi,
                display: self.display,
                wifi: (),
                rtc: self.rtc,
            },
        )
    }

    pub fn rtc_peripheral(self) -> (RTCUtils, Board<Backlight, ScreenSpi, Display, Wifi, ()>) {
        (
            self.rtc,
            Board {
                screen_backlight: self.screen_backlight,
                screen_spi: self.screen_spi,
                display: self.display,
                wifi: self.wifi,
                rtc: (),
            },
        )
    }
}

impl<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
    Board<Backlight, ScreenSpi, Display, Wifi, RTCUtils>
{
    pub fn backlight<T>(self, p: T) -> Board<T, ScreenSpi, Display, Wifi, RTCUtils> {
        Board {
            screen_backlight: p,
            screen_spi: self.screen_spi,
            display: self.display,
            wifi: self.wifi,
            rtc: self.rtc,
        }
    }

    pub fn screen_spi<T>(self, s: T) -> Board<Backlight, T, Display, Wifi, RTCUtils> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: s,
            display: self.display,
            wifi: self.wifi,
            rtc: self.rtc,
        }
    }
    pub fn display<T>(self, d: T) -> Board<Backlight, ScreenSpi, T, Wifi, RTCUtils> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: self.screen_spi,
            display: d,
            wifi: self.wifi,
            rtc: self.rtc,
        }
    }
    pub fn wifi<T>(self, w: T) -> Board<Backlight, ScreenSpi, Display, T, RTCUtils> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: self.screen_spi,
            display: self.display,
            wifi: w,
            rtc: self.rtc,
        }
    }
    pub fn rtc<T>(self, r: T) -> Board<Backlight, ScreenSpi, Display, Wifi, T> {
        Board {
            screen_backlight: self.screen_backlight,
            screen_spi: self.screen_spi,
            display: self.display,
            wifi: self.wifi,
            rtc: r,
        }
    }
}

const SLINT_TARGET_FPS: u64 = 25;
const SLINT_FRAME_DURATION_MS: u64 = 1000 / SLINT_TARGET_FPS;

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
            log::trace!("slint drawing time {}", total);
            if !window.has_active_animations() {
                if let Some(duration) = slint::platform::duration_until_next_timer_update() {
                    let millis = duration.as_millis().try_into().unwrap();
                    log::trace!("will sleep for {}ms", millis);
                    Timer::after(Duration::from_millis(millis)).await;
                } else {
                    // https://github.com/slint-ui/slint/discussions/3994
                    // use a stream to await next event, and don't sleep for ever.
                }
            } else {
                let pause_for_target_fps =
                    SLINT_FRAME_DURATION_MS as i32 - total.to_millis() as i32;

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
}

#[embassy_executor::task]
async fn say_hello() {
    loop {
        let stats = esp_alloc::HEAP.stats();
        // HeapStats implements the Display and defmt::Format traits, so you can pretty-print the heap stats.
        log::info!("{}", stats);
        Timer::after_millis(1000).await;
    }
}
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
async fn update_timer(main_window: Rc<Recipe>, rtc: Rc<RTCUtils>) {
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
        main_window.set_name(current_time.format("%H:%M:%S").to_shared_string());
        let actual = current_time.second() % 10;
        if (actual == 0 && actual != last_value) {
            visible = !visible;
            last_value = actual;
        }
        main_window.set_show_monsters(visible);

        Timer::after_millis(100).await;
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

        Timer::after(Duration::from_secs(15)).await; // Every 15 minutes
    }
}

#[embassy_executor::task]
async fn net_task(mut runner: Runner<'static, WifiDevice<'static, WifiStaDevice>>) {
    runner.run().await
}

fn inner_main(
    spawner: Spawner,
    board: Board<types::LedChannel, (), (), Wifi, RTCUtils>,
    main_window: Rc<Recipe>,
) {
    log::info!("spawning post_init");
    let (led_channel, board) = board.backlight_peripheral();
    let (wifi, board) = board.wifi_peripheral();
    let (rtc, board) = board.rtc_peripheral();
    let rtc_rc = Rc::new(rtc);

    let _ = spawner.spawn(fade_screen(led_channel));
    let _ = spawner.spawn(update_timer(main_window.clone(), rtc_rc.clone()));

    let _ = spawner.spawn(connection(wifi.controller)).ok();
    let _ = spawner.spawn(net_task(wifi.runner)).ok();

    let _ = spawner.spawn(ntp_task(wifi.stack, rtc_rc.clone()));
}

#[embassy_executor::task]
async fn embassy_main(
    spawner: Spawner,
    window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
    // post_init: &'static (dyn Fn(Spawner, Board<types::LedChannel, (), ()>, Rc<Recipe>) -> ()),
    ui_state: &'static Signal<CriticalSectionRawMutex, Rc<Recipe>>,
) -> ! {
    log::info!("init embassy");
    let board = boards::init();
    log::info!("init embassy done");
    let (display, board) = board.display_peripheral();

    use embedded_graphics::geometry::OriginDimensions;

    let size = display.size();
    let size = slint::PhysicalSize::new(size.width, size.height);

    window.borrow().as_ref().unwrap().set_size(size);

    spawner.spawn(refresh_screen(window, display)).unwrap();

    let ui = ui_state.wait().await;
    inner_main(spawner, board, ui);
    loop {
        Timer::after(Duration::from_millis(5_000)).await;
    }
}

pub struct EspEmbassyBackend {
    window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
    // post_init: &'static (dyn Fn(Spawner, Board<types::LedChannel, (), ()>) -> ()),
    ui_state: &'static Signal<CriticalSectionRawMutex, Rc<Recipe>>, // will be initialized after backend
}
impl EspEmbassyBackend {
    pub fn new(
        // post_init: &'static (
        //     impl Fn(Spawner, Board<types::LedChannel, (), ()>) -> (),
        //     Rc<Recipe>,
        // ),
        ui_state: &'static Signal<CriticalSectionRawMutex, Rc<Recipe>>,
    ) -> Self {
        Self {
            window: RefCell::default(),
            // post_init: post_init,
            ui_state,
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
            spawner.must_spawn(embassy_main(spawner, self.window.clone(), self.ui_state));
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
