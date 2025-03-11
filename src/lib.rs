#![cfg_attr(feature = "mcu", no_std)]

extern crate alloc;

#[cfg(feature = "mcu")]
pub mod board;
pub mod boards;
pub mod controller;
pub mod ntp;
pub mod slintplatform;
pub mod wifi;
