use core::net::{IpAddr, SocketAddr};

use alloc::rc::Rc;
use chrono::{offset, DateTime, TimeDelta, Utc};
use embassy_net::{udp::UdpSocket, Stack};
use embassy_time::{Duration, Instant, Timer};
use smoltcp::{storage::PacketMetadata, wire::DnsQueryType};
use sntpc::{get_time, NtpContext, NtpTimestampGenerator};

use crate::controller::Hardware;

const NTP_SERVER: &str = "pool.ntp.org";

#[derive(Copy, Clone)]
struct Timestamp {
    duration: Duration,
    offset: DateTime<Utc>,
}
impl Timestamp {
    fn new(offset: DateTime<Utc>) -> Timestamp {
        Timestamp {
            duration: Duration::default(),
            offset,
        }
    }
}

impl<'a> NtpTimestampGenerator for Timestamp {
    fn init(&mut self) {
        self.duration = Duration::from_micros(
            (self.offset + TimeDelta::milliseconds(Instant::now().as_millis().try_into().unwrap()))
                .timestamp_micros()
                .try_into()
                .unwrap(),
        );
        log::info!("duration: {}ms", self.duration.as_millis());
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

pub struct NtpClient<'a> {
    stack: Stack<'a>,
    context: NtpContext<Timestamp>,
}

impl<'a> NtpClient<'a> {
    pub fn new(stack: Stack<'a>) -> NtpClient<'a> {
        NtpClient {
            stack,
            context: NtpContext::new(Timestamp::new(DateTime::from_timestamp_nanos(0))),
        }
    }

    pub async fn run(mut self) {
        let stack = self.stack;
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
        let addr: IpAddr = ntp_addrs[0].into();
        loop {
            let result = get_time(SocketAddr::from((addr, 123)), &socket, self.context).await;

            match result {
                Ok(time) => {
                    let datetime = DateTime::from_timestamp(
                        time.sec().into(),
                        (time.sec_fraction() as u64 * 1_000_000_000 / 4_294_967_296) as u32,
                    )
                    .unwrap();

                    self.context = NtpContext::new(Timestamp::new(datetime));
                    if (first) {
                        start = datetime;
                        now = DateTime::from_timestamp_micros(Instant::now().as_micros() as i64)
                            .unwrap();
                        // self.hardware.set_current_time(datetime.naive_local());
                        // rtc.ds1307
                        //     .lock()
                        //     .await
                        //     .set_datetime(&datetime.naive_local())
                        //     .ok();
                        first = false;
                    }
                    // let delta = rtc.rtc.current_time().and_utc() - start;
                    // let delta_main_clock =
                    //     DateTime::from_timestamp_micros(Instant::now().as_micros() as i64).unwrap()
                    //         - now;
                    let delta_ntp = datetime - start;
                    log::info!(
                        "Time: {:?}, offset: {}, roundtrip: {}",
                        datetime,
                        time.offset(),
                        time.roundtrip()
                    );
                    // log::info!(
                    //     "Elapsed rtc: {}us, cpu: {}us, ntp: {}us",
                    //     delta,
                    //     delta_main_clock,
                    //     delta_ntp
                    // );
                    // log::info!(
                    //     "Deltas rtc/ntp: {}, cpu/ntp: {}",
                    //     delta_ntp - delta,
                    //     delta_ntp - delta_main_clock
                    // );
                }
                Err(e) => {
                    log::error!("Error getting time: {:?}", e);
                }
            }

            Timer::after(Duration::from_secs(15 * 60)).await; // Every 15 minutes
        }
    }

    pub fn get_date_time(self) -> DateTime<Utc> {
        let mut context = self.context.clone();
        context.timestamp_gen.init();
        DateTime::from_timestamp(
            context.timestamp_gen.timestamp_sec().try_into().unwrap(),
            context.timestamp_gen.timestamp_subsec_micros() * 1000,
        )
        .unwrap()
    }
}
