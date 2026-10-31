//! The time a glide and the flourishes sample. Real in the helper, stepped
//! by hand in tests, so a frame schedule can be asserted without sleeping.

use std::time::{Duration, Instant};

pub(crate) trait Clock {
    fn now_ms(&self) -> u64;
    fn sleep(&mut self, duration: Duration);
}

pub(crate) struct WallClock(Instant);

impl WallClock {
    pub(crate) fn start() -> Self {
        Self(Instant::now())
    }
}

impl Clock for WallClock {
    fn now_ms(&self) -> u64 {
        self.0.elapsed().as_millis() as u64
    }

    fn sleep(&mut self, duration: Duration) {
        std::thread::sleep(duration);
    }
}
