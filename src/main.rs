#![no_std]
#![no_main]


use embedded_hal_bus::spi::ExclusiveDevice;
use esp_backtrace as _;
use hal::{delay::Delay, gpio::{Io, Level, Output}, prelude::*,

          spi::{master::Spi, SpiMode}};

extern crate alloc;

use core::mem::MaybeUninit;


use embedded_graphics::{
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{Circle, Primitive, PrimitiveStyle, Triangle},
};

// Provides the parallel port and display interface builders
use display_interface_spi::SPIInterface;

// Provides the Display builder
use mipidsi::Builder;
use mipidsi::models::ILI9341Rgb565;
use mipidsi::options::{Orientation, Rotation};

fn init_heap() {
    const HEAP_SIZE: usize = 32 * 1024;
    static mut HEAP: MaybeUninit<[u8; HEAP_SIZE]> = MaybeUninit::uninit();

    unsafe {
        esp_alloc::HEAP.add_region(esp_alloc::HeapRegion::new(
            HEAP.as_mut_ptr() as *mut u8,
            HEAP_SIZE,
            esp_alloc::MemoryCapability::Internal.into(),
        ));
    }
}

fn configure_screen() {
    let peripherals = hal::init(hal::Config::default());
    let mut delay = Delay::new();
    let io = Io::new(peripherals.GPIO, peripherals.IO_MUX);
    // Define the Data/Command select pin as a digital output
    let dc = Output::new(io.pins.gpio7, Level::Low);
    // Define the reset pin as digital outputs and make it high
    let mut rst = Output::new(io.pins.gpio9, Level::Low);
    rst.set_high();

    // Define the SPI pins and create the SPI interface
    let sck = io.pins.gpio4;
    let miso = io.pins.gpio5;
    let mosi = io.pins.gpio6;
    let cs = io.pins.gpio2;
    let spi = Spi::new(peripherals.SPI2, 60u32.MHz(), SpiMode::Mode0).with_pins(
        sck,
        mosi,
        miso,
        hal::gpio::NoPin,
    );
    let cs_output = Output::new(cs, Level::High);
    let spi_device = ExclusiveDevice::new_no_delay(spi, cs_output).unwrap();
    let mut buffer = [0_u8; 512];

    // Define the display interface with no chip select
    let di = SPIInterface::new(spi_device, dc);
    // Define the display from the display interface and initialize it
    let mut display = Builder::new(ILI9341Rgb565, di)
        .reset_pin(rst)
        .color_order(mipidsi::options::ColorOrder::Bgr)
        .orientation( Orientation::new().rotate(Rotation::Deg180))
        .init(&mut delay)
        .unwrap();

    // Make the display all black
    display.clear(Rgb565::BLACK).unwrap();

    // Draw a smiley face with white eyes and a red mouth
    draw_smiley(&mut display).unwrap();
}

fn draw_smiley<T: DrawTarget<Color=Rgb565>>(display: &mut T) -> Result<(), T::Error> {
    // Draw the left eye as a circle located at (50, 100), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 100), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw the right eye as a circle located at (50, 200), with a diameter of 40, filled with white
    Circle::new(Point::new(50, 200), 40)
        .into_styled(PrimitiveStyle::with_fill(Rgb565::WHITE))
        .draw(display)?;

    // Draw an upside down red triangle to represent a smiling mouth
    Triangle::new(
        Point::new(130, 140),
        Point::new(130, 200),
        Point::new(160, 170),
    )
        .into_styled(PrimitiveStyle::with_fill(Rgb565::RED))
        .draw(display)?;

    // Cover the top part of the mouth with a black triangle so it looks closed instead of open
    Triangle::new(
        Point::new(130, 150),
        Point::new(130, 190),
        Point::new(150, 170),
    )
        .into_styled(PrimitiveStyle::with_fill(Rgb565::BLACK))
        .draw(display)?;

    Ok(())
}

#[entry]
fn main() -> ! {

    let delay = Delay::new();
    init_heap();

    esp_println::logger::init_logger_from_env();

    configure_screen();
    loop {
        log::info!("Hello world!");
        delay.delay(500.millis());
    }
}
