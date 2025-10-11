#[link(name = "c", kind = "dylib")]
unsafe extern "C" {
    pub unsafe fn mach_absolute_time() -> u64;
    #[link_name = "AbsoluteToNanoseconds"]
    pub unsafe fn absolute_to_nanoseconds(ticks: u64) -> u64;
}

#[derive(Clone, Copy, Debug)]
pub struct Instant {
    timestamp: u64,
}

#[derive(Clone, Copy, Debug)]
pub struct Duration {
    nanoseconds: u64,
}

impl Instant {
    pub fn now() -> Self {
        unsafe {
            Self {
                timestamp: mach_absolute_time(),
            }
        }
    }
}

impl core::ops::Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Duration {
        let timestamp_diff = self.timestamp.saturating_sub(rhs.timestamp);
        Duration {
            nanoseconds: unsafe { absolute_to_nanoseconds(timestamp_diff) },
        }
    }
}

impl Duration {
    pub fn as_secs_f64(&self) -> f64 {
        self.nanoseconds as f64 / 1_000_000_000.0
    }
}
