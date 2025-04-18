use core::fmt::Display;

use alloc::vec::{self, Vec};

use chrono::{DateTime, Utc};
use color_hex::color_from_hex;
use i_slint_core::graphics::{GradientStop, LinearGradientBrush};
use micromath::F32Ext;

use slint::{Brush, Color};
use slint_generated::TimeOfDay;
use spa::FloatOps;

pub enum MicroMathFloatOps {}
impl FloatOps for MicroMathFloatOps {
    fn sin(x: f64) -> f64 {
        (x as f32).sin() as f64
    }
    fn cos(x: f64) -> f64 {
        (x as f32).cos() as f64
    }
    fn tan(x: f64) -> f64 {
        (x as f32).tan() as f64
    }
    fn asin(x: f64) -> f64 {
        (x as f32).asin() as f64
    }
    fn acos(x: f64) -> f64 {
        (x as f32).acos() as f64
    }
    fn atan(x: f64) -> f64 {
        (x as f32).atan() as f64
    }
    fn atan2(y: f64, x: f64) -> f64 {
        (y as f32).atan2((x as f32)) as f64
    }
    fn trunc(x: f64) -> f64 {
        (x as f32).trunc() as f64
    }
}

macro_rules! color_from_hex_str {
    ($hex:expr) => {
        Color::from_rgb_u8(
            color_from_hex!($hex)[0],
            color_from_hex!($hex)[1],
            color_from_hex!($hex)[2],
        )
    };
}

#[derive(Debug, Clone, Copy)]
pub struct Gradient {
    start: Color,
    steps: [(Color, u8); 1],
    end: Color,
}

impl Display for Gradient {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "Gradient(start: {}, end: {}", self.start, self.end)
    }
}

#[derive(Debug, Clone, Copy)]
pub enum SkyMoment {
    NIGHT,
    DAWN,
    SUNRISE,
    MORNING,
    AFTERNOON,
    SUNSET,
    DUSK,
}

impl Display for SkyMoment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SkyMoment::NIGHT => write!(f, "SkyMoment::NIGHT"),
            SkyMoment::DAWN => write!(f, "SkyMoment::DAWN"),
            SkyMoment::SUNRISE => write!(f, "SkyMoment::SUNRISE"),
            SkyMoment::MORNING => write!(f, "SkyMoment::MORNING"),
            SkyMoment::AFTERNOON => write!(f, "SkyMoment::AFTERNOON"),
            SkyMoment::SUNSET => write!(f, "SkyMoment::SUNSET"),
            SkyMoment::DUSK => write!(f, "SkyMoment::DUSK"),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Sky {
    gradient: Gradient,
    moment: SkyMoment,
    start_angle: f32,
    end_angle: f32,
}

impl Display for Sky {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "Sky(gradient: {}, start: {}, end: {})",
            self.gradient, self.start_angle, self.end_angle
        )
    }
}

pub const SKY: [Sky; 22] = [
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#0C0A18"),
            steps: [(color_from_hex_str!("#0C0A18"), 1)],
            end: color_from_hex_str!("#020111"),
        },
        moment: SkyMoment::NIGHT,
        start_angle: -270.0,
        end_angle: -10.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#1E1E2A"),
            steps: [(color_from_hex_str!("#10101E"), 10)],
            end: color_from_hex_str!("#020111"),
        },
        moment: SkyMoment::DAWN,
        start_angle: -10.0,
        end_angle: -8.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#3A3A52"),
            steps: [(color_from_hex_str!("#313147"), 10)],
            end: color_from_hex_str!("#020111"),
        },
        moment: SkyMoment::DAWN,
        start_angle: -8.0,
        end_angle: -6.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#505074"),
            steps: [(color_from_hex_str!("#2A2A3A"), 90)],
            end: color_from_hex_str!("#232331"),
        },
        moment: SkyMoment::DAWN,
        start_angle: -6.0,
        end_angle: -4.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#8A76AB"),
            steps: [(color_from_hex_str!("#6C6EA6"), 20)],
            end: color_from_hex_str!("#444463"),
        },
        moment: SkyMoment::DAWN,
        start_angle: -4.0,
        end_angle: -2.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#CD82A0"),
            steps: [(color_from_hex_str!("#7071AA"), 50)],
            end: color_from_hex_str!("#504F73"),
        },
        moment: SkyMoment::DAWN,
        start_angle: -2.0,
        end_angle: 0.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#E5AED0"),
            steps: [(color_from_hex_str!("#8985BF"), 30)],
            end: color_from_hex_str!("#777BBF"),
        },
        moment: SkyMoment::SUNRISE,
        start_angle: 0.0,
        end_angle: 2.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#EBB2B1"),
            steps: [(color_from_hex_str!("#BDB0C3"), 40)],
            end: color_from_hex_str!("#8AADD8"),
        },
        moment: SkyMoment::SUNRISE,
        start_angle: 2.0,
        end_angle: 4.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#B0B7EB"),
            steps: [(color_from_hex_str!("#A6E3FE"), 30)],
            end: color_from_hex_str!("#95C7F8"),
        },
        moment: SkyMoment::SUNRISE,
        start_angle: 4.0,
        end_angle: 6.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#94DFFF"),
            steps: [(color_from_hex_str!("#AAE6FF"), 80)],
            end: color_from_hex_str!("#B4E9FF"),
        },
        moment: SkyMoment::MORNING,
        start_angle: 6.0,
        end_angle: 20.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#67D1FB"),
            steps: [(color_from_hex_str!("#92DFFD"), 90)],
            end: color_from_hex_str!("#9BE2FE"),
        },
        moment: SkyMoment::MORNING,
        start_angle: 20.0,
        end_angle: 80.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#1E528E"),
            steps: [(color_from_hex_str!("#2A84B8"), 90)],
            end: color_from_hex_str!("#2B8BBD"),
        },
        moment: SkyMoment::AFTERNOON,
        start_angle: 80.0,
        end_angle: 1.5,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#5B7983"),
            steps: [(color_from_hex_str!("#1F538E"), 30)],
            end: color_from_hex_str!("#236FA7"),
        },
        moment: SkyMoment::SUNSET,
        start_angle: 1.5,
        end_angle: 1.0,
    },
    Sky {
        //
        gradient: Gradient {
            start: color_from_hex_str!("#9DA671"),
            steps: [(color_from_hex_str!("#2A5B88"), 50)],
            end: color_from_hex_str!("#1F528E"),
        },
        moment: SkyMoment::SUNSET,
        start_angle: 1.0,
        end_angle: 0.5,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#E9CE5D"),
            steps: [(color_from_hex_str!("#798E7A"), 50)],
            end: color_from_hex_str!("#2B5A8B"),
        },
        moment: SkyMoment::SUNSET,
        start_angle: 0.0, // sunset
        end_angle: -4.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#B26339"),
            steps: [(color_from_hex_str!("#D5BC60"), 30)],
            end: color_from_hex_str!("#264D75"),
        },
        moment: SkyMoment::SUNSET,
        start_angle: -4.0,
        end_angle: -4.5,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#491C09"),
            steps: [
                // (color_from_hex_str!("#B5490F"), 10),
                (color_from_hex_str!("#BE732E"), 30),
            ],
            end: color_from_hex_str!("#25414F"),
        },
        moment: SkyMoment::DUSK,
        start_angle: -4.5,
        end_angle: -5.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#341505"),
            steps: [(color_from_hex_str!("#7F3814"), 20)],
            end: color_from_hex_str!("#071B26"),
        },
        moment: SkyMoment::DUSK,
        start_angle: -5.0,
        end_angle: -5.5,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#381508"),
            steps: [(color_from_hex_str!("#52210B"), 20)],
            end: color_from_hex_str!("#010A10"),
        },
        moment: SkyMoment::DUSK,
        start_angle: -5.5,
        end_angle: -6.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#4A1D06"),
            steps: [(color_from_hex_str!("#3D1805"), 10)],
            end: color_from_hex_str!("#090401"),
        },
        moment: SkyMoment::DUSK,
        start_angle: -6.0,
        end_angle: -6.5,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#0C0A18"),
            steps: [(color_from_hex_str!("#0C0A18"), 1)],
            end: color_from_hex_str!("#020111"),
        },
        moment: SkyMoment::DUSK,
        start_angle: -6.5,
        end_angle: -8.0,
    },
    Sky {
        gradient: Gradient {
            start: color_from_hex_str!("#0C0A18"),
            steps: [(color_from_hex_str!("#0C0A18"), 1)],
            end: color_from_hex_str!("#020111"),
        },
        moment: SkyMoment::NIGHT,
        start_angle: -8.0,
        end_angle: -270.0,
    },
];

fn mix_colors(lower: &Color, upper: &Color, mix_factor: f32) -> Color {
    log::info!("Mixing: {} and {} at {}", lower, upper, mix_factor * 100.0);
    upper.mix(lower, mix_factor)
}

impl Sky {
    fn midle_angle(self) -> f32 {
        self.start_angle + (self.end_angle - self.start_angle) / 2.0
    }
}

type NightFactor = f32;

pub fn get_slint_gradient(
    date_time: DateTime<Utc>,
) -> (TimeOfDay, NightFactor, LinearGradientBrush) {
    let pos = spa::solar_position::<MicroMathFloatOps>(date_time, 48.866667, 2.333333).unwrap();

    let angle = 90.0 - pos.zenith_angle as f32; // in degrees

    let current_idx = if (pos.azimuth > 180.0) {
        SKY.iter()
            .rev()
            .position(|s| {
                log::info!("testing : {} with {}", s, angle);
                s.start_angle > (angle as f32)
            })
            .unwrap()
    } else {
        SKY.iter()
            .position(|s| s.start_angle > (angle as f32))
            .unwrap()
    };

    let idx = if (pos.azimuth > 180.0) {
        SKY.len() - 1 - current_idx as usize
    } else {
        current_idx as usize - 1
    };
    log::info!(
        "date: {}, angle: {}, currentidx: {}, idx: {}, pos: {}",
        date_time,
        angle,
        current_idx,
        idx,
        pos.azimuth
    );

    let current_sky = SKY[idx];

    let position_in_interval =
        (angle - current_sky.start_angle) / (current_sky.end_angle - current_sky.start_angle);

    let (upper_sky, lower_sky) = if (position_in_interval < 0.5) {
        log::info!(
            "angle: {}, position_in_interval: {}, upper_idx: {}, lower_idx: {}",
            angle,
            position_in_interval,
            idx,
            idx - 1
        );
        (SKY[idx], SKY[idx - 1])
    } else {
        log::info!(
            "angle: {}, position_in_interval: {}, upper_idx: {}, lower_idx: {}",
            angle,
            position_in_interval,
            idx + 1,
            idx
        );
        (SKY[idx + 1], SKY[idx])
    };
    log::info!(
        "angle: {}, position_in_interval: {}, upper.middle: {}, lower.middle: {}",
        angle,
        position_in_interval,
        upper_sky.midle_angle(),
        lower_sky.midle_angle()
    );

    let mix_factor =
        (angle - lower_sky.midle_angle()) / (upper_sky.midle_angle() - lower_sky.midle_angle());

    let start_color = mix_colors(
        &lower_sky.gradient.start,
        &upper_sky.gradient.start,
        mix_factor,
    );

    let end_color = mix_colors(&lower_sky.gradient.end, &upper_sky.gradient.end, mix_factor);

    log::info!(
        "angle: {}, currentidx: {}, mix_factor: {}, sky: {}",
        angle,
        idx,
        mix_factor,
        lower_sky
    );

    let (tod) = match current_sky.moment {
        SkyMoment::NIGHT => TimeOfDay::NIGHT,
        SkyMoment::DAWN => TimeOfDay::TWILIGHT,
        SkyMoment::SUNRISE => TimeOfDay::TWILIGHT,
        SkyMoment::MORNING => TimeOfDay::DAY,
        SkyMoment::AFTERNOON => TimeOfDay::DAY,
        SkyMoment::SUNSET => TimeOfDay::TWILIGHT,
        SkyMoment::DUSK => TimeOfDay::TWILIGHT,
    };

    let corrected_angle = if (pos.azimuth > 180.0) {
        270.0 + angle
    } else {
        90.0 + angle
    };

    let night_factor = ((angle - (0.5)) / (-8.0 - 0.5)).clamp(0.0, 1.0);
    log::info!(
        "corrected angle: {}, night_factor: {}, tod: {}, start: {}, end: {}",
        corrected_angle,
        night_factor * 100.0,
        current_sky.moment,
        start_color,
        end_color
    );

    (
        tod,
        night_factor,
        LinearGradientBrush::new(
            corrected_angle,
            [
                GradientStop {
                    color: start_color,
                    position: 0.3,
                },
                GradientStop {
                    color: end_color,
                    position: 0.7,
                },
            ],
        ),
    )
}
