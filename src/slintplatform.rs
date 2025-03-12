use alloc::rc::Rc;

use embassy_time::Instant;
use slint::platform::software_renderer::MinimalSoftwareWindow;

pub struct EspEmbassyBackend {
    window: Rc<MinimalSoftwareWindow>,
}
impl EspEmbassyBackend {
    pub fn new(window: Rc<MinimalSoftwareWindow>) -> Self {
        Self { window }
    }
}

impl slint::platform::Platform for EspEmbassyBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = self.window.clone();
        Ok(window)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        Instant::now().duration_since(Instant::from_secs(0)).into()
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        log::debug!("{}", arguments);
    }
}
