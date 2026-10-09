//! The day and the time of day a moment is by this machine's clock: what the
//! list of saves says of when each was written, as the PSP's own list does.
//!
//! The standard library tells a moment only as time since 1970 by the clock
//! of the world. Where the system can be asked what that is here it is
//! (Windows: `FileTimeToSystemTime` and `SystemTimeToTzSpecificLocalTime`, the
//! only code of the port that is not safe Rust); elsewhere the world's own
//! day and time are given.

use std::time::{SystemTime, UNIX_EPOCH};

/// A day and a time of day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Moment {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl Moment {
    /// As the list of saves writes it: day, month and year, and the time.
    pub fn written(&self) -> String {
        format!("{:02}/{:02}/{:04} {:02}:{:02}", self.day, self.month, self.year, self.hour, self.minute)
    }
}

/// The day and time so many seconds after 1970 began, by the clock of the
/// world (the days are counted off in eras of four hundred years).
fn of_the_world(seconds: u64) -> Moment {
    let (days, rest) = (seconds / 86_400, seconds % 86_400);
    // From the first of March of the year nought, which puts a leap day last.
    let days = days + 719_468;
    let (era, day_of_era) = (days / 146_097, days % 146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let month = if month_from_march < 10 { month_from_march + 3 } else { month_from_march - 9 };
    let year = year_of_era + era * 400 + u64::from(month <= 2);
    Moment { year: year as u32, month: month as u32, day: day as u32, hour: (rest / 3_600) as u32, minute: (rest % 3_600 / 60) as u32 }
}

#[cfg(windows)]
mod system {
    use super::Moment;

    #[repr(C)]
    #[derive(Default)]
    struct SystemTime {
        year: u16,
        month: u16,
        day_of_week: u16,
        day: u16,
        hour: u16,
        minute: u16,
        second: u16,
        milliseconds: u16,
    }

    #[repr(C)]
    struct FileTime {
        low: u32,
        high: u32,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn FileTimeToSystemTime(file: *const FileTime, system: *mut SystemTime) -> i32;
        fn SystemTimeToTzSpecificLocalTime(zone: *const core::ffi::c_void, universal: *const SystemTime, local: *mut SystemTime) -> i32;
    }

    /// The seconds from 1601, where the system's count of time begins, to 1970.
    const TO_1970: u64 = 11_644_473_600;

    /// The day and time here so many seconds after 1970 began.
    pub fn here(seconds: u64) -> Option<Moment> {
        let ticks = (seconds + TO_1970).checked_mul(10_000_000)?;
        let file = FileTime { low: ticks as u32, high: (ticks >> 32) as u32 };
        let (mut world, mut local) = (SystemTime::default(), SystemTime::default());
        // SAFETY: each call reads the one struct it is given and writes the
        // other, both of the layouts the system documents and both alive for
        // the call; a null zone asks for the zone the machine is in.
        let done = unsafe { FileTimeToSystemTime(&file, &mut world) != 0 && SystemTimeToTzSpecificLocalTime(core::ptr::null(), &world, &mut local) != 0 };
        done.then(|| Moment { year: local.year.into(), month: local.month.into(), day: local.day.into(), hour: local.hour.into(), minute: local.minute.into() })
    }
}

#[cfg(not(windows))]
mod system {
    pub fn here(_seconds: u64) -> Option<super::Moment> {
        None
    }
}

/// The day and the time of day `moment` is here; none for one before 1970.
pub fn moment(moment: SystemTime) -> Option<Moment> {
    let seconds = moment.duration_since(UNIX_EPOCH).ok()?.as_secs();
    Some(system::here(seconds).unwrap_or_else(|| of_the_world(seconds)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn a_moment_is_a_day_and_a_time_of_day() {
        assert_eq!(of_the_world(0), Moment { year: 1970, month: 1, day: 1, hour: 0, minute: 0 });
        // A leap day, the day after it, and the last minute of a year.
        assert_eq!(of_the_world(951_782_400 + 13 * 3_600 + 5 * 60), Moment { year: 2000, month: 2, day: 29, hour: 13, minute: 5 });
        assert_eq!(of_the_world(951_868_800), Moment { year: 2000, month: 3, day: 1, hour: 0, minute: 0 });
        assert_eq!(of_the_world(1_798_761_599), Moment { year: 2026, month: 12, day: 31, hour: 23, minute: 59 });
        assert_eq!(of_the_world(1_791_497_700).written(), "08/10/2026 22:15");
        // Here it is that or within a day of it, whatever zone this is.
        let here = moment(UNIX_EPOCH + Duration::from_secs(1_791_497_700)).unwrap();
        assert!(here.year == 2026 && here.month == 10 && (7..=9).contains(&here.day), "{here:?}");
        assert_eq!(moment(UNIX_EPOCH - Duration::from_secs(1)), None);
    }
}
