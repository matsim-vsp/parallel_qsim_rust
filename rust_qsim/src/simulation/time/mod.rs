use std::fmt::{Display, Formatter};
use std::num::NonZeroU32;
use std::ops::{Add, AddAssign, Sub};
use std::time::Duration;

const NANOS_PER_SECOND: u128 = 1_000_000_000;
const NANOS_PER_SECOND_U64: u64 = 1_000_000_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Tick(u64);

impl Tick {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn new(value: u64) -> Self {
        Self(value)
    }

    pub fn value(self) -> u64 {
        self.0
    }

    pub fn next(self) -> Self {
        Self(self.0.saturating_add(1))
    }

    pub fn is_multiple_of(self, rhs: u64) -> bool {
        self.0.is_multiple_of(rhs)
    }

    pub fn checked_add(self, rhs: Self) -> Option<Self> {
        self.0.checked_add(rhs.0).map(Self)
    }

    pub fn saturating_add(self, rhs: Self) -> Self {
        Self(self.0.saturating_add(rhs.0))
    }

    pub fn saturating_sub(self, rhs: Self) -> Self {
        Self(self.0.saturating_sub(rhs.0))
    }
}

impl Add for Tick {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl From<u64> for Tick {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<u32> for Tick {
    fn from(value: u32) -> Self {
        Self::new(value as u64)
    }
}

impl From<i32> for Tick {
    fn from(value: i32) -> Self {
        assert!(value >= 0, "tick values must be non-negative");
        Self::new(value as u64)
    }
}

impl From<usize> for Tick {
    fn from(value: usize) -> Self {
        Self::new(value as u64)
    }
}

impl From<Tick> for u64 {
    fn from(value: Tick) -> Self {
        value.value()
    }
}

impl PartialEq<u32> for Tick {
    fn eq(&self, other: &u32) -> bool {
        self.value() == *other as u64
    }
}

impl PartialEq<Tick> for u32 {
    fn eq(&self, other: &Tick) -> bool {
        *self as u64 == other.value()
    }
}

impl AddAssign for Tick {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for Tick {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct SimTime(Duration);

impl SimTime {
    pub fn from_duration(duration: Duration) -> Self {
        Self(duration)
    }

    pub fn from_millis(millis: u64) -> Self {
        Self(Duration::from_millis(millis))
    }

    pub fn from_nanos(nanos: u64) -> Self {
        Self(Duration::from_nanos(nanos))
    }

    // TODO rename
    pub fn from_u32_seconds(seconds: u32) -> Self {
        Self(Duration::from_secs(seconds as u64))
    }

    pub fn as_nanos(self) -> u128 {
        self.0.as_nanos()
    }

    pub fn as_millis(self) -> u128 {
        self.0.as_millis()
    }

    pub fn as_u32_seconds(self) -> u32 {
        self.0.as_secs().try_into().unwrap_or(u32::MAX)
    }

    pub fn as_duration(self) -> Duration {
        self.0
    }

    pub fn duration_since(self, earlier: SimTime) -> Duration {
        self.0.saturating_sub(earlier.0)
    }

    pub fn saturating_add(self, duration: Duration) -> Self {
        Self(self.0.saturating_add(duration))
    }

    pub fn saturating_sub(self, duration: Duration) -> Self {
        Self(self.0.saturating_sub(duration))
    }

    pub fn parse_hh_mm_ss(input: &str) -> Result<Self, String> {
        let mut parts = input.split(':');
        let hours = parts
            .next()
            .ok_or_else(|| "missing hours".to_string())?
            .parse::<u64>()
            .map_err(|_| "invalid hours".to_string())?;
        let minutes = parts
            .next()
            .ok_or_else(|| "missing minutes".to_string())?
            .parse::<u64>()
            .map_err(|_| "invalid minutes".to_string())?;
        let seconds_part = parts.next().ok_or_else(|| "missing seconds".to_string())?;
        if parts.next().is_some() {
            return Err("too many ':' separators".to_string());
        }

        let (seconds, millis) = match seconds_part.split_once('.') {
            Some((seconds, nanos)) => {
                let seconds = seconds
                    .parse::<u64>()
                    .map_err(|_| "invalid seconds".to_string())?;
                let nanos = normalize_nanos(nanos)?;
                (seconds, nanos)
            }
            None => (
                seconds_part
                    .parse::<u64>()
                    .map_err(|_| "invalid seconds".to_string())?,
                0,
            ),
        };

        let total_seconds = hours
            .saturating_mul(60)
            .saturating_add(minutes)
            .saturating_mul(60)
            .saturating_add(seconds);
        let total_nanos = total_seconds
            .saturating_mul(NANOS_PER_SECOND_U64)
            .saturating_add(millis);
        Ok(Self(Duration::from_nanos(total_nanos)))
    }

    pub fn parse_decimal_seconds(input: &str) -> Result<Self, String> {
        let (seconds, nanos) = match input.split_once('.') {
            Some((seconds, nanos)) => {
                let seconds = seconds
                    .parse::<u64>()
                    .map_err(|_| "invalid seconds".to_string())?;
                let nanos = normalize_nanos_truncate(nanos)?;
                (seconds, nanos)
            }
            None => (
                input
                    .parse::<u64>()
                    .map_err(|_| "invalid seconds".to_string())?,
                0,
            ),
        };

        Ok(Self::from_nanos(
            seconds
                .saturating_mul(NANOS_PER_SECOND_U64)
                .saturating_add(nanos),
        ))
    }

    pub fn format_decimal_seconds(self) -> String {
        let total_nanos = self.0.as_nanos();
        let seconds = total_nanos / NANOS_PER_SECOND;
        let nanos = total_nanos % NANOS_PER_SECOND;
        if nanos == 0 {
            seconds.to_string()
        } else {
            format!("{seconds}.{nanos:09}")
                .trim_end_matches('0')
                .to_string()
        }
    }

    pub fn format_hh_mm_ss_trimmed(self) -> String {
        let total_nanos = self.0.as_nanos();
        let total_seconds = total_nanos / NANOS_PER_SECOND;
        let nanos = total_nanos % NANOS_PER_SECOND;
        let hours = total_seconds / 3600;
        let minutes = (total_seconds % 3600) / 60;
        let seconds = total_seconds % 60;

        if nanos == 0 {
            format!("{hours:02}:{minutes:02}:{seconds:02}")
        } else {
            format!("{hours:02}:{minutes:02}:{seconds:02}.{nanos:09}")
                .trim_end_matches('0')
                .to_string()
        }
    }
}

impl Display for SimTime {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.format_hh_mm_ss_trimmed())
    }
}

#[allow(unused)]
fn normalize_nanos(input: &str) -> Result<u64, String> {
    if input.is_empty() || input.len() > 9 || !input.chars().all(|c| c.is_ascii_digit()) {
        return Err("expected 1-9 decimal digits for nanoseconds".to_string());
    }

    let mut nanos = input.to_string();
    while nanos.len() < 9 {
        nanos.push('0');
    }
    nanos
        .parse::<u64>()
        .map_err(|_| "invalid nanoseconds".to_string())
}

fn normalize_nanos_truncate(input: &str) -> Result<u64, String> {
    if input.is_empty() || !input.chars().all(|c| c.is_ascii_digit()) {
        return Err("expected decimal digits for nanoseconds".to_string());
    }

    let mut nanos = input.chars().take(9).collect::<String>();
    while nanos.len() < 9 {
        nanos.push('0');
    }
    nanos
        .parse::<u64>()
        .map_err(|_| "invalid nanoseconds".to_string())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SimClock {
    ticks_per_second: NonZeroU32,
}

impl SimClock {
    pub(crate) fn new(ticks_per_second: u32) -> Self {
        Self {
            ticks_per_second: NonZeroU32::new(ticks_per_second)
                .expect("ticks_per_second must be > 0"),
        }
    }

    pub(crate) fn ticks_per_second(self) -> u32 {
        self.ticks_per_second.get()
    }

    pub(crate) fn tick_length(self) -> Duration {
        Duration::from_secs_f64(1.0 / self.ticks_per_second() as f64)
    }

    pub(crate) fn time_to_tick(self, time: SimTime) -> Tick {
        let ticks = div_ceil(
            time.as_nanos() * self.ticks_per_second() as u128,
            NANOS_PER_SECOND,
        );
        Tick(ticks as u64)
    }

    pub(crate) fn tick_to_time(self, tick: Tick) -> SimTime {
        let nanos = tick.value() as u128 * NANOS_PER_SECOND / self.ticks_per_second() as u128;
        SimTime::from_nanos(nanos as u64)
    }

    pub(crate) fn u32_seconds_to_tick(self, seconds: u32) -> Tick {
        self.time_to_tick(SimTime::from_u32_seconds(seconds))
    }

    pub(crate) fn tick_to_u32_seconds(self, tick: Tick) -> u32 {
        self.tick_to_time(tick).as_u32_seconds()
    }

    #[cfg(test)]
    pub(crate) fn time_to_u32_seconds(self, time: SimTime) -> u32 {
        time.as_u32_seconds()
    }

    #[allow(unused)]
    pub(crate) fn seconds_to_ticks_ceil(self, seconds: f64) -> Tick {
        let ticks = (seconds * self.ticks_per_second() as f64).ceil() as u64;
        Tick(ticks)
    }

    pub(crate) fn seconds_to_travel_ticks(self, seconds: f64) -> Tick {
        // Vehicles spend one extra node-processing step on a link after the queue travel time.
        // Using the floored in-queue duration preserves the legacy observed event times.
        Tick::new((seconds * self.ticks_per_second() as f64).floor().max(0.0) as u64)
    }
}

fn div_ceil(left: u128, right: u128) -> u128 {
    left.div_ceil(right)
}

#[cfg(test)]
mod tests {
    use super::{NANOS_PER_SECOND_U64, SimClock, SimTime, Tick};
    use std::time::Duration;

    #[test]
    fn round_trip_with_one_tick_per_second() {
        let clock = SimClock::new(1);
        let tick = clock.u32_seconds_to_tick(42);
        assert_eq!(tick, Tick::new(42));
        assert_eq!(clock.tick_to_u32_seconds(tick), 42);
        assert_eq!(clock.tick_to_time(tick), SimTime::from_u32_seconds(42));
    }

    #[test]
    fn converts_subsecond_time_to_tick_and_back() {
        let clock = SimClock::new(10);
        let time = SimTime::from_duration(Duration::from_millis(350));
        let tick = clock.time_to_tick(time);
        assert_eq!(tick, Tick::new(4));
        assert_eq!(clock.tick_to_u32_seconds(tick), 0);
        assert_eq!(
            clock.tick_to_time(tick),
            SimTime::from_duration(Duration::from_millis(400))
        );
    }

    #[test]
    fn outward_seconds_are_truncated() {
        let clock = SimClock::new(10);
        let time = SimTime::from_duration(Duration::from_millis(1999));
        assert_eq!(clock.time_to_u32_seconds(time), 1);
    }

    #[test]
    fn travel_ticks_preserve_observed_link_duration() {
        let clock = SimClock::new(1);
        assert_eq!(
            clock.seconds_to_travel_ticks(10_000.0 / 27.78),
            Tick::new(359)
        );
        assert_eq!(clock.seconds_to_travel_ticks(100.0), Tick::new(100));
        assert_eq!(clock.seconds_to_travel_ticks(0.5), Tick::zero());

        let subsecond_clock = SimClock::new(10);
        assert_eq!(
            subsecond_clock.seconds_to_travel_ticks(10_000.0 / 27.78),
            Tick::new(3599)
        );
        assert_eq!(subsecond_clock.seconds_to_travel_ticks(0.3), Tick::new(3));
    }

    #[test]
    fn formats_decimal_seconds_without_fraction_for_full_seconds() {
        let time = SimTime::from_u32_seconds(42);
        assert_eq!(time.format_decimal_seconds(), "42");
    }

    #[test]
    fn formats_decimal_seconds_with_trimmed_nanos() {
        let nanos = SimTime::from_nanos(42_000_000_001);
        let precise = SimTime::from_nanos(42_123_456_789);
        let trailing_zero = SimTime::from_nanos(42_120_000_000);

        assert_eq!(nanos.format_decimal_seconds(), "42.000000001");
        assert_eq!(precise.format_decimal_seconds(), "42.123456789");
        assert_eq!(trailing_zero.format_decimal_seconds(), "42.12");
    }

    #[test]
    fn formats_hh_mm_ss_without_fraction_for_full_seconds() {
        let time = SimTime::from_u32_seconds(7 * 3600 + 30 * 60 + 15);
        assert_eq!(time.format_hh_mm_ss_trimmed(), "07:30:15");
    }

    #[test]
    fn formats_hh_mm_ss_with_trimmed_nanos() {
        let nanos = SimTime::from_nanos(
            (7 * 3600 + 30 * 60 + 15) as u64 * NANOS_PER_SECOND_U64 + 250_000_000,
        );
        let precise = SimTime::from_nanos(
            (7 * 3600 + 30 * 60 + 15) as u64 * NANOS_PER_SECOND_U64 + 123_456_789,
        );
        let trailing_zero = SimTime::from_nanos(
            (7 * 3600 + 30 * 60 + 15) as u64 * NANOS_PER_SECOND_U64 + 120_000_000,
        );

        assert_eq!(nanos.format_hh_mm_ss_trimmed(), "07:30:15.25");
        assert_eq!(precise.format_hh_mm_ss_trimmed(), "07:30:15.123456789");
        assert_eq!(trailing_zero.format_hh_mm_ss_trimmed(), "07:30:15.12");
    }

    #[test]
    fn parses_decimal_seconds_with_up_to_nine_digits() {
        assert_eq!(
            SimTime::parse_decimal_seconds("42.1").unwrap(),
            SimTime::from_nanos(42_100_000_000)
        );
        assert_eq!(
            SimTime::parse_decimal_seconds("42.123456789").unwrap(),
            SimTime::from_nanos(42_123_456_789)
        );
    }

    #[test]
    fn parses_and_truncates_decimal_seconds_to_nanos() {
        let parsed = SimTime::parse_decimal_seconds("42.1234567899").unwrap();
        assert_eq!(parsed, SimTime::from_nanos(42_123_456_789));
    }

    #[test]
    fn parses_hh_mm_ss_with_subseconds() {
        let parsed = SimTime::parse_hh_mm_ss("07:30:15.250000001").unwrap();
        assert_eq!(
            parsed,
            SimTime::from_nanos(
                (7 * 3600 + 30 * 60 + 15) as u64 * NANOS_PER_SECOND_U64 + 250_000_001,
            )
        );
    }
}
