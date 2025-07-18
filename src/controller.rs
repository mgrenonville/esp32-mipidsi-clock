// Copyright © 2025 David Haig
// SPDX-License-Identifier: MIT

use core::{
    cell::RefCell,
    fmt::{Debug, Display},
};

use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use chrono::{DateTime, Timelike, Utc};
use chrono_tz::{Europe::Paris, Tz};
use embassy_sync::{
    blocking_mutex::raw::CriticalSectionRawMutex,
    blocking_mutex::{CriticalSectionMutex, Mutex},
    channel::Channel,
    signal::Signal,
    waitqueue::WakerRegistration,
};
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics::prelude::Point;
use i_slint_core::graphics::LinearGradientBrush;
use log::{debug, error};
use slint::{Brush, ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer, ToSharedString};
use slint_generated::{Globals, MonsterEnv, Recipe, TimeOfDay, WifiState};

use log::warn;
use tiny_skia::{Color, FillRule, Mask, Paint, PathBuilder, Pixmap, Transform};

use crate::moon::Moon;

#[cfg(feature = "mcu")]
use crate::board::Board;

#[derive(Debug, Clone)]
pub enum Action {
    MultipleActions(Vec<Action>),
    HardwareUserBtnPressed(bool),
    TouchscreenToggleBtn(bool),
    WifiStateUpdate(WifiState),
    TimeOfDayUpdate(TimeOfDay, Moon),
    UpdateTime(DateTime<Tz>),
    ShowMonster(bool),
    StartCountDown(DateTime<Tz>, u8),
}

#[cfg(feature = "mcu")]
type ActionChannelType =
    Channel<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Action, 4>;

#[cfg(feature = "simulator")]
type ActionChannelType =
    Channel<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Action, 2>;

type RefreshScreenChannelType =
    Channel<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Action, 1>;

pub static ACTION: ActionChannelType = Channel::new();
pub static REFRESH_SIGNAL: RefreshScreenChannelType = Channel::new();
pub static WAKER: WakerRegistration = WakerRegistration::new();
static SOME_SIGNAL: Signal<CriticalSectionRawMutex, ()> = Signal::new();

struct MoonAndTime {
    version: DateTime<Tz>,
}
impl Debug for MoonAndTime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "MoonAndTime(version: {})", self.version)
    }
}

static CURRENT_MOON: CriticalSectionMutex<RefCell<Option<MoonAndTime>>> =
    CriticalSectionMutex::new(RefCell::new(Option::None));

pub trait WallClock {
    async fn get_date_time(&self) -> DateTime<Utc>;
    async fn set_date_time(&self, datetime: chrono::DateTime<Utc>);
}
// see mcu::hardware or simulator::hardware modules for impl
// depending on features used
pub trait Hardware {
    // fn green_led_set_high(&mut self) {}

    // fn green_led_set_low(&mut self) {}
}
#[cfg(feature = "mcu")]
impl Hardware for Board {}

pub const MOON_SIZE: usize = 34;

pub struct Controller<'a, Hardware, WallClock> {
    main_window: &'a Recipe,
    hardware: Hardware,
    wall_clock: Rc<WallClock>,
    current_sky: CriticalSectionMutex<RefCell<Option<MoonAndTime>>>,
}

impl<'a, H, WC> Controller<'a, H, WC>
where
    H: Hardware,
    WC: WallClock,
{
    pub fn new(main_window: &'a Recipe, hardware: H, wall_clock: Rc<WC>) -> Self {
        Self {
            main_window,
            hardware,
            wall_clock,
            current_sky: CriticalSectionMutex::new(RefCell::new(Option::None)),
        }
    }

    pub async fn run(&mut self) {
        self.set_action_event_handlers();

        loop {
            let action = ACTION.receive().await;

            match self.process_action(action).await {
                Ok(()) => {
                    // all good
                }
                Err(e) => {
                    error!("process action: {:?}", e);
                }
            }
        }
    }

    pub async fn process_action(&mut self, action: Action) -> Result<(), ()> {
        let globals = self.main_window.global::<Globals>();

        log::info!("process_action: {:?}", action);

        // Refresh has to be asked BEFORE updating
        // see https://github.com/slint-ui/slint/discussions/3994#discussioncomment-7680584
        match REFRESH_SIGNAL.try_send(action.clone()) {
            Ok(_) => {
                log::info!(
                    "{} - trigger refresh: {:?}",
                    Instant::now().as_millis(),
                    action
                );
            }
            Err(_) => debug!("refresh action queue full, could not add: {:?}", action),
        };
        Timer::after(Duration::from_millis(1)).await;
        match action.clone() {
            Action::HardwareUserBtnPressed(is_pressed) => {
                // globals.set_hardware_user_btn_pressed(is_pressed);
            }
            Action::TouchscreenToggleBtn(on) => {
                if on {
                    // self.hardware.green_led_set_low();
                } else {
                    // self.hardware.green_led_set_high()
                }
            }
            Action::StartCountDown(current_time, duration) => {
                let d = chrono::Duration::seconds(duration.into());
                let stops_at = current_time.checked_add_signed(d).unwrap();
                globals.set_countdown(stops_at.timestamp());
                globals.set_countdown_total_duration(duration.into());
            }
            Action::WifiStateUpdate(wifi_state) => globals.set_wifi_state(wifi_state),
            Action::UpdateTime(current_time) => {
                globals.set_current_time(current_time.timestamp());

                let up_to_date_sky = self.current_sky.lock(|r| {
                    r.borrow()
                        .as_ref()
                        .is_some_and(|m| (current_time - m.version).num_seconds().abs() < 60)
                });
                if (!up_to_date_sky) {
                    self.current_sky.lock(|r| {
                        r.replace(Option::Some(MoonAndTime {
                            version: current_time,
                        }))
                    });

                    log::info!("Generating sky and position for 1m");
                    let (tod, night_factor, brush) =
                        crate::sky::get_slint_gradient(current_time.to_utc());
                    globals.set_night_factor(night_factor);
                    globals.set_time_of_day(tod);

                    let mut point = Point { x: 125, y: 188 }; // outside
                    let mut env = slint_generated::MonsterEnv::OUTSIDE;

                    let local_time = current_time.with_timezone(&Paris);
                    if ((local_time.hour() >= 20 || local_time.hour() < 8) || night_factor > 0.25) {
                        point = Point { x: 195, y: 138 }; // in house
                        env = slint_generated::MonsterEnv::HOUSE;
                    }

                    if (local_time.hour() >= 21 || local_time.hour() < 7) {
                        env = slint_generated::MonsterEnv::SLEEPING;
                    }

                    globals.set_sky_brush(Brush::LinearGradient(brush));
                    globals.set_monster_position(slint_generated::MonsterPosition {
                        env: env,
                        x: point.x,
                        y: point.y,
                    });
                }

                let updated_moon = CURRENT_MOON.lock(|r| {
                    r.borrow()
                        .as_ref()
                        .is_some_and(|m| current_time.timestamp() - m.version.timestamp() < 3600)
                });

                if (!updated_moon) {
                    CURRENT_MOON.lock(|r| {
                        r.replace(Option::Some(MoonAndTime {
                            version: current_time,
                        }))
                    });
                    log::info!("Generating moon for 1h");
                    let buff = Moon::new(current_time.to_utc()).build_image();
                    globals.set_moon(Image::from_rgba8(buff));
                }
            }
            Action::ShowMonster(monster) => {
                globals.set_monster_visibility(monster);
            }
            Action::TimeOfDayUpdate(tod, moon) => {
                globals.set_time_of_day(tod);

                let mut full_moon_paint = Paint::default();
                full_moon_paint.set_color_rgba8(255, 246, 153, 255);
                full_moon_paint.anti_alias = true;

                let mut pixmap = Pixmap::new(34, 34).unwrap();

                let mut computed = (34.0 * (moon.illumination));
                if (moon.phase > 0.5) {
                    computed = computed + 34. / 2. as f32
                } else {
                    computed = 34. / 2. - computed as f32
                }
                let shadow =
                    PathBuilder::from_circle(computed, (34.0 / 2.0) as f32, (34 / 2) as f32)
                        .unwrap();

                log::info!(
                    "phase: {}, computed: {}, emoji: {}",
                    moon.phase,
                    computed,
                    moon.phase_emoji()
                );

                let full_moon = PathBuilder::from_circle(
                    (34.0 / 2.0) as f32,
                    (34.0 / 2.0) as f32,
                    (34 / 2) as f32,
                )
                .unwrap();

                let mut mask = Mask::new(34, 34).unwrap();
                mask.fill_path(
                    &shadow,
                    FillRule::Winding,
                    true,
                    Transform::from_rotate_at(-25.0, 34. / 2., 34. / 2.),
                );
                mask.invert();

                // let t = Transform::from_rotate(-20.0);
                // pixmap.fill(Color::from_rgba8(2, 4, 38, 255));
                pixmap.fill_path(
                    &full_moon,
                    &full_moon_paint,
                    FillRule::Winding,
                    Transform::identity(),
                    Some(&mask),
                );

                let i = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
                    pixmap.data_mut(),
                    MOON_SIZE.try_into().unwrap(),
                    MOON_SIZE.try_into().unwrap(),
                );
                globals.set_moon(Image::from_rgba8(i));
            }
            Action::MultipleActions(actions) => {
                for a in actions.iter() {
                    let _ = Box::pin(self.process_action(a.clone())).await;
                }
            }
        }

        Ok(())
    }

    // user initiated action event handlers
    fn set_action_event_handlers(&self) {
        let globals = self.main_window.global::<Globals>();
        globals.on_format_countdown(|now, stops| {
            let now = chrono::DateTime::from_timestamp(now, 0).unwrap();
            let stops_at = chrono::DateTime::from_timestamp(stops, 0).unwrap();
            let duration = stops_at - now;
            format!(
                "{:02}:{:02}",
                duration.num_seconds() / 60,
                duration.num_seconds() % 60
            )
            .to_shared_string()
        });

        globals.on_format_time(|now| {
            let datetime = chrono::DateTime::from_timestamp(now, 0).unwrap();
            datetime
                .with_timezone(&Paris)
                .format("%H:%M")
                .to_shared_string()
        });
        globals.set_countdown(0);
    }
}

pub fn send_action(a: Action) {
    // use non-blocking try_send here because this function needs is called from sync code (the gui callbacks)
    match ACTION.try_send(a) {
        Ok(_) => {
            // see loop in `fn run()` for dequeue
        }
        Err(a) => {
            // this could happen because the controller is slow to respond or we are making too many requests
            warn!("user action queue full, could not add: {:?}", a)
        }
    }
}

pub async fn refresh_screen() -> Action {
    let r = REFRESH_SIGNAL.receive().await;
    REFRESH_SIGNAL.clear();
    r
}

pub fn empty_refresh_screen() {
    REFRESH_SIGNAL.try_receive().ok();
}
