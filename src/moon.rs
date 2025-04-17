use core::f32::consts::TAU;

use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use micromath::F32Ext;
use slint::{Rgba8Pixel, SharedPixelBuffer};
use tiny_skia::{FillRule, Mask, Paint, PathBuilder, Pixmap, Transform};

/// The period of the lunar orbit in days.
pub const ORBIT_PERIOD: f32 = 29.53058770576;

/// The offset for the lunar orbit calculations.
pub const ORBIT_OFFSET: f32 = 2451550.26;

/// The period of the lunar distance in days.
pub const DISTANCE_PERIOD: f32 = 27.55454988;

/// The offset for the lunar distance calculations.
pub const DISTANCE_OFFSET: f32 = 2451562.2;

/// The base value for calculating lunation.
pub const LUNATION_BASE: f32 = 2423436.6115277777;

/// The mean radius of the Earth in kilometers.
pub const EARTH_RADIUS_KM: f32 = 6371.0084;

/// Represents a lunar phase with name, emoji and start and end fractions.
#[derive(Debug, Clone, Copy)]
pub struct Phase {
    /// Name of the lunar phase.
    pub name: &'static str,
    /// Emoji representing the lunar phase.
    pub emoji: &'static str,
    /// Start fraction of the lunar phase.
    pub start: f32,
    /// End fraction of the lunar phase.
    pub end: f32,
}
pub const PHASES: [Phase; 8] = [
    Phase {
        emoji: "🌑",
        name: "New Moon",
        start: 0.0,
        end: 0.02,
    },
    Phase {
        emoji: "🌒",
        name: "Waxing Crescent",
        start: 0.02,
        end: 0.22,
    },
    Phase {
        emoji: "🌓",
        name: "First Quarter",
        start: 0.22,
        end: 0.27,
    },
    Phase {
        emoji: "🌔",
        name: "Waxing Gibbous",
        start: 0.27,
        end: 0.47,
    },
    Phase {
        emoji: "🌕",
        name: "Full Moon",
        start: 0.47,
        end: 0.52,
    },
    Phase {
        emoji: "🌖",
        name: "Waning Gibbous",
        start: 0.52,
        end: 0.72,
    },
    Phase {
        emoji: "🌗",
        name: "Last Quarter",
        start: 0.72,
        end: 0.77,
    },
    Phase {
        emoji: "🌘",
        name: "Waning Crescent",
        start: 0.77,
        end: 1.0,
    },
];

/// Represents information about the moon, including its julian date, phase,
/// age, illumination, distance, and lunation.
#[derive(Debug, Clone, Copy)]
pub struct Moon {
    /// A continuous count of days and fractions since noon Universal Time on January 1, 4713 BC
    pub julian_date: f32,
    /// Phase of the moon.
    pub phase: f32,
    /// Age of the moon.
    pub age: f32,
    /// Illumination of the moon (0 to 1 where 1 is a full moon).
    pub illumination: f32,
    /// Distance of the moon in earth radii.
    pub distance: f32,
    /// Lunation number.
    pub lunation: u16,
}

/// Calculates the Julian date of the moon based on the provided `SystemTime`.
fn julian_date(time: DateTime<Utc>) -> f32 {
    let unix_time: f32 = time.timestamp() as f32;

    unix_time / 86400. + 2440587.5
}
/// Calculates the illumination of the moon based on the given phase.
fn illumination(phase: f32) -> f32 {
    0.5 * (1.0 - (TAU * phase).cos())
}

/// Calculates the lunation (lunar month count) based on the provided Julian date.
fn lunation(julian_date: f32) -> u16 {
    (1. + (julian_date - LUNATION_BASE) / ORBIT_PERIOD).floor() as u16
}

/// Calculates the phase of the moon based on the provided Julian date.
fn phase(julian_date: f32) -> f32 {
    ((julian_date - ORBIT_OFFSET) / ORBIT_PERIOD).fract()
}
/// Calculates the distance phase of the moon based on the provided Julian date.
fn distance_phase(julian_date: f32) -> f32 {
    ((julian_date - DISTANCE_OFFSET) / DISTANCE_PERIOD).fract()
}
fn distance(phase: f32, julian_date: f32) -> f32 {
    let distance_p = distance_phase(julian_date);
    let distance_p_tau: f32 = TAU * distance_p;
    let p_tau: f32 = 2.0 * TAU * phase;
    let p_distance_tau_diff = p_tau - distance_p_tau;

    60.4 - 3.3 * distance_p_tau.cos() - 0.6 * (p_distance_tau_diff).cos() - 0.5 * (p_tau).cos()
}
impl Moon {
    pub fn test(phase: f32) -> Moon {
        Moon {
            julian_date: 0.0,
            age: 0.0,
            phase: phase,
            illumination: 0.0,
            distance: 0.0,
            lunation: 0,
        }
    }

    pub fn new(time: DateTime<Utc>) -> Moon {
        let julian_date: f32 = julian_date(time);
        let phase: f32 = phase(julian_date);
        let age: f32 = phase * ORBIT_PERIOD;
        let illumination: f32 = illumination(phase);
        let lunation: u16 = lunation(julian_date);
        let distance: f32 = distance(phase, julian_date);

        Moon {
            julian_date,
            age,
            phase,
            illumination,
            distance,
            lunation,
        }
    }

    /// Returns the distance of the moon in kilometers.
    pub fn distance_km(&self) -> f32 {
        self.distance * EARTH_RADIUS_KM
    }

    /// Checks if the moon is in the waning phase.
    pub fn is_waning(&self) -> bool {
        self.age < 0.5
    }
    /// Checks if the moon is in the waxing phase.
    pub fn is_waxing(&self) -> bool {
        self.age > 0.5
    }
    /// Returns the name of the moon phase.
    pub fn phase_name(&self) -> &'static str {
        for phase in PHASES.iter() {
            if self.phase >= phase.start && self.phase < phase.end {
                return phase.name;
            }
        }
        "Unknown"
    }

    /// Returns the emoji representation of the moon phase.
    pub fn phase_emoji(&self) -> &'static str {
        for phase in PHASES.iter() {
            if self.phase >= phase.start && self.phase < phase.end {
                return phase.emoji;
            }
        }
        "Unknown"
    }

    pub fn build_image(self) -> SharedPixelBuffer<Rgba8Pixel> {
        let mut full_moon_paint = Paint::default();
        full_moon_paint.set_color_rgba8(255, 246, 153, 255);
        full_moon_paint.anti_alias = true;

        let mut pixmap = Pixmap::new(34, 34).unwrap();

        let mut computed = (34.0 * (self.illumination));
        if (self.phase > 0.5) {
            computed = computed + 34. / 2. as f32
        } else {
            computed = 34. / 2. - computed as f32
        }
        let shadow =
            PathBuilder::from_circle(computed, (34.0 / 2.0) as f32, (34 / 2) as f32).unwrap();

        log::info!(
            "phase: {}, computed: {}, emoji: {}",
            self.phase,
            computed,
            self.phase_emoji()
        );

        let full_moon =
            PathBuilder::from_circle((34.0 / 2.0) as f32, (34.0 / 2.0) as f32, (34 / 2) as f32)
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

        let i = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(pixmap.data_mut(), 34, 34);
        i
    }
}
