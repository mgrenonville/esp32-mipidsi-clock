#![cfg_attr(feature = "mcu", no_std)]

extern crate alloc;

#[cfg(feature = "mcu")]
pub mod board;
#[cfg(feature = "mcu")]
pub mod boards;

pub mod controller;
#[cfg(feature = "mcu")]
pub mod ntp;
pub mod slintplatform;
#[cfg(feature = "mcu")]
pub mod wifi;
