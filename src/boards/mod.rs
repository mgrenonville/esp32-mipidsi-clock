use crate::board::types::DisplayImpl;
use embedded_graphics::{pixelcolor::raw::RawU16, prelude::RgbColor};
use mipidsi::{interface::InterfacePixelFormat, models::Model};

pub struct DrawBuffer<'a, Display> {
    pub display: Display,
    pub buffer: &'a mut [slint::platform::software_renderer::Rgb565Pixel],
}

impl<M> slint::platform::software_renderer::LineBufferProvider
    for &mut DrawBuffer<'_, DisplayImpl<M>>
where
    M: Model,
    M::ColorFormat: InterfacePixelFormat<u8>,
    M::ColorFormat: RgbColor,
    M::ColorFormat: From<RawU16>,
{
    type TargetPixel = slint::platform::software_renderer::Rgb565Pixel;

    fn process_line(
        &mut self,
        line: usize,
        range: core::ops::Range<usize>,
        render_fn: impl FnOnce(&mut [slint::platform::software_renderer::Rgb565Pixel]),
    ) {
        let buffer = &mut self.buffer[range.clone()];
        log::debug!(
            "Redraw l: {}, range: {}-{} ({})",
            line,
            range.start,
            range.end,
            range.end - range.start
        );
        render_fn(buffer);

        // We send empty data just to get the device in the right window
        self.display
            .set_pixels(
                range.start as u16,
                line as _,
                (range.end - 1) as u16, // Range are inclusive /!\
                line as u16,
                buffer.iter().map(|x| RawU16::new(x.0).into()),
            )
            .unwrap();
    }
}
