use embassy_futures::join;
use embassy_time::{Duration, Timer};
use esp_wifi::wifi::{ClientConfiguration, Configuration, WifiController, WifiEvent, WifiState};

// pub trait MyWifiController {
//     async fn run();
// }

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

pub struct EspEmbassyWifiController<'a> {
    ctrl: WifiController<'a>,
}

impl<'a> EspEmbassyWifiController<'a> {
    pub fn new<'b>(ctrl: WifiController<'b>) -> EspEmbassyWifiController<'b> {
        EspEmbassyWifiController::<'b> { ctrl }
    }

    pub async fn connection(&mut self) {
        log::info!("start connection task");
        log::info!("Device capabilities: {:?}", self.ctrl.capabilities());
        loop {
            match esp_wifi::wifi::wifi_state() {
                WifiState::StaConnected => {
                    // wait until we're no longer connected
                    self.ctrl.wait_for_event(WifiEvent::StaDisconnected).await;
                    Timer::after(Duration::from_millis(5000)).await
                }
                _ => {}
            }
            if !matches!(self.ctrl.is_started(), Ok(true)) {
                let client_config = Configuration::Client(ClientConfiguration {
                    ssid: SSID.try_into().unwrap(),
                    password: PASSWORD.try_into().unwrap(),
                    ..Default::default()
                });
                self.ctrl.set_configuration(&client_config).unwrap();
                log::info!("Starting wifi");
                self.ctrl.start_async().await.unwrap();
                log::info!("Wifi started!");
            }
            log::info!("About to connect to {} with {}...", SSID, PASSWORD);

            match self.ctrl.connect_async().await {
                Ok(_) => log::info!("Wifi connected!"),
                Err(e) => {
                    log::info!("Failed to connect to wifi: {e:?}");
                    Timer::after(Duration::from_millis(5000)).await
                }
            }
        }
    }
}
