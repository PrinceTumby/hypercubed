use super::interrupts;
use core::sync::atomic::Ordering::Relaxed as RelaxedOrdering;
use core::sync::atomic::{AtomicBool, AtomicU32};

#[expect(unused)]
const NS_PER_BUS_CLOCK: u64 = 6_781_684;
const NS_PER_BUS_CLOCK_DIV_16: u64 = 108_506_944;

const TIMER_COUNTS: [*mut u32; 3] = [
    0x10000000 as *mut u32,
    0x10000800 as *mut u32,
    0x10001000 as *mut u32,
];

const TIMER_MODES: [*mut u32; 3] = [
    0x10000010 as *mut u32,
    0x10000810 as *mut u32,
    0x10001010 as *mut u32,
];

const NS_PER_INSTANT_CLOCK_TICK: u64 = NS_PER_BUS_CLOCK_DIV_16;

static CURRENT_INSTANT_CLOCK_UPPER: AtomicU32 = AtomicU32::new(0);
static INSTANT_CLOCK_INITIALISED: AtomicBool = AtomicBool::new(false);

extern "C" fn instant_timer_overflow_handler(_cause: i32) -> i32 {
    unsafe {
        CURRENT_INSTANT_CLOCK_UPPER.fetch_add(1, RelaxedOrdering);
        // Clear overflow interrupt flag.
        *TIMER_MODES[2] |= 1 << 11;
        // Error code?
        0
    }
}

pub unsafe fn init_instant_clock() {
    unsafe {
        assert!(!INSTANT_CLOCK_INITIALISED.load(RelaxedOrdering));
        CURRENT_INSTANT_CLOCK_UPPER.store(0, RelaxedOrdering);
        INSTANT_CLOCK_INITIALISED.store(true, RelaxedOrdering);
        interrupts::add_handler(
            interrupts::InterruptType::Timer2,
            instant_timer_overflow_handler,
        )
        .unwrap();
        // Set bits 0-1 = 1 - Set speed to bus clock / 16
        // Set bit 7 - Enable timer
        // Set bit 9 - Enable overflow interrupts
        // Set bit 11 - Clear overflow interrupt flag
        *TIMER_MODES[2] = 0xA81;
    }
}

pub unsafe fn get_current_instant_clock_timestamp() -> u64 {
    unsafe {
        assert!(INSTANT_CLOCK_INITIALISED.load(RelaxedOrdering));
        let mut upper = CURRENT_INSTANT_CLOCK_UPPER.load(RelaxedOrdering);
        let mut lower = *TIMER_COUNTS[2];
        // Check if an overflow has happened while we're reading the clock.
        let second_upper = CURRENT_INSTANT_CLOCK_UPPER.load(RelaxedOrdering);
        if upper != second_upper {
            upper = second_upper;
            lower = *TIMER_COUNTS[2];
        }
        ((upper as u64) << 16) | (lower as u64)
    }
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
                timestamp: get_current_instant_clock_timestamp(),
            }
        }
    }
}

impl core::ops::Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Duration {
        let timestamp_diff = self.timestamp.saturating_sub(rhs.timestamp);
        Duration {
            nanoseconds: timestamp_diff * NS_PER_INSTANT_CLOCK_TICK,
        }
    }
}

impl Duration {
    pub fn as_secs_f64(&self) -> f64 {
        self.nanoseconds as f64 / 1_000_000_000.0
    }
}
