use alloc::rc::Rc;

use esp_hal::time;
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
        core::time::Duration::from_micros(time::now().duration_since_epoch().to_micros())
    }

    fn debug_log(&self, arguments: core::fmt::Arguments) {
        esp_println::println!("{}", arguments);
    }
}
