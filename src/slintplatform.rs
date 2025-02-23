use core::{
    cell::RefCell,
    net::{IpAddr, SocketAddr},
};

use alloc::rc::Rc;
use chrono::{DateTime, Timelike};
use chrono_tz::Europe::Paris;
use ds1307::DateTimeAccess;
use embassy_executor::Spawner;
use embassy_futures::select::select;
use embassy_net::{
    udp::{PacketMetadata, UdpSocket},
    Runner, Stack,
};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Duration, Instant, Timer};
use esp_hal::{ledc::channel::ChannelIFace, time};
use esp_hal_embassy::Executor;
use esp_wifi::wifi::{
    ClientConfiguration, Configuration, WifiController, WifiDevice, WifiEvent, WifiStaDevice,
    WifiState,
};
use mipidsi::models::ST7789;
use slint::{platform::software_renderer::MinimalSoftwareWindow, ToSharedString};
use smoltcp::wire::DnsQueryType;
use sntpc::{get_time, NtpContext, NtpTimestampGenerator};

use crate::{
    board::{
        types::{self, DisplayImpl, LedChannel, RTCUtils},
        Board, Wifi,
    },
    singleton,
};

use crate::{
    boards::{self, slintdraw::DrawBuffer},
    Recipe,
};

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

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");
const NTP_SERVER: &str = "pool.ntp.org";

const SLINT_TARGET_FPS: u64 = 25;
const SLINT_FRAME_DURATION_MS: u64 = 1000 / SLINT_TARGET_FPS;

#[embassy_executor::task]
async fn refresh_screen(
    window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
    display: DisplayImpl<ST7789>,
    refresh_signal: Rc<Signal<CriticalSectionRawMutex, ()>>,
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
                    select(
                        refresh_signal.wait(),
                        Timer::after(Duration::from_millis(millis)),
                    )
                    .await;
                } else {
                    // https://github.com/slint-ui/slint/discussions/3994
                    refresh_signal.wait().await;
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
async fn update_timer(
    main_window: Rc<Recipe>,
    rtc: Rc<RTCUtils>,
    refresh_signal: Rc<Signal<CriticalSectionRawMutex, ()>>,
) {
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
        if (actual != last_value) {
            visible = !visible;
        }
        last_value = actual;
        main_window.set_show_monsters(visible);
        log::debug!(
            "Setting visible monster: {} (actual: {}, last_value{}, current_time: {})",
            visible,
            actual,
            last_value,
            current_time
        );
        // trigger refresh
        refresh_signal.signal(());
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

        Timer::after(Duration::from_secs(15 * 60)).await; // Every 15 minutes
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
    refresh_signal: Rc<Signal<CriticalSectionRawMutex, ()>>,
) {
    log::info!("spawning post_init");
    let (led_channel, board) = board.backlight_peripheral();
    let (wifi, board) = board.wifi_peripheral();
    let (rtc, board) = board.rtc_peripheral();
    let rtc_rc = Rc::new(rtc);

    let _ = spawner.spawn(fade_screen(led_channel));
    let _ = spawner.spawn(update_timer(
        main_window.clone(),
        rtc_rc.clone(),
        refresh_signal.clone(),
    ));

    let _ = spawner.spawn(connection(wifi.controller)).ok();
    let _ = spawner.spawn(net_task(wifi.runner)).ok();

    let _ = spawner.spawn(ntp_task(wifi.stack, rtc_rc.clone()));
    let _ = spawner.spawn(wifi_status_task(
        wifi.stack,
        main_window.clone(),
        refresh_signal,
    ));
}

#[embassy_executor::task]
async fn wifi_status_task(
    stack: Stack<'static>,
    main_window: Rc<Recipe>,
    refresh_signal: Rc<Signal<CriticalSectionRawMutex, ()>>,
) {
    loop {
        if (stack.is_link_up()) {
            if (stack.is_config_up()) {
                main_window.set_wifi_state(crate::WifiState::OK);
            } else {
                main_window.set_wifi_state(crate::WifiState::LINKUP);
            }
        } else {
            main_window.set_wifi_state(crate::WifiState::STARTING);
        }

        refresh_signal.signal(());
        if (stack.is_config_up()) {
            Timer::after(Duration::from_millis(50)).await;
        } else {
            Timer::after(Duration::from_secs(10)).await;
        }
    }
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

    let refresh_signal = Rc::new(Signal::<CriticalSectionRawMutex, ()>::new());

    let size = display.size();
    let size = slint::PhysicalSize::new(size.width, size.height);

    window.borrow().as_ref().unwrap().set_size(size);

    spawner
        .spawn(refresh_screen(window, display, refresh_signal.clone()))
        .unwrap();

    let ui = ui_state.wait().await;
    inner_main(spawner, board, ui, refresh_signal);
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
