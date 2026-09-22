use windows::Win32::Foundation::{FILETIME, SYSTEMTIME};

/// Vanilla's embedded `SYSTEMTIME` (same field order/size as `windows::Win32::Foundation::SYSTEMTIME`,
/// confirmed against that crate's own definition) - kept as a distinct type rather than the
/// real `windows` struct directly so enclosing structs remain `#[repr(C)]`-stable independent of
/// that crate's own attributes; conversions to/from the real `SYSTEMTIME`/`FILETIME`
/// via [`Systemtime::to_win32`]/[`Systemtime::from_win32`] for `SystemTimeToFileTime`/
/// `FileTimeToSystemTime` round-trips.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(C)]
pub struct Systemtime {
    pub w_year: u16,
    pub w_month: u16,
    pub w_day_of_week: u16,
    pub w_day: u16,
    pub w_hour: u16,
    pub w_minute: u16,
    pub w_second: u16,
    pub w_milliseconds: u16,
}

impl Systemtime {
    pub fn to_win32(self) -> SYSTEMTIME {
        SYSTEMTIME {
            wYear: self.w_year,
            wMonth: self.w_month,
            wDayOfWeek: self.w_day_of_week,
            wDay: self.w_day,
            wHour: self.w_hour,
            wMinute: self.w_minute,
            wSecond: self.w_second,
            wMilliseconds: self.w_milliseconds,
        }
    }

    pub fn from_win32(value: SYSTEMTIME) -> Self {
        Self {
            w_year: value.wYear,
            w_month: value.wMonth,
            w_day_of_week: value.wDayOfWeek,
            w_day: value.wDay,
            w_hour: value.wHour,
            w_minute: value.wMinute,
            w_second: value.wSecond,
            w_milliseconds: value.wMilliseconds,
        }
    }
}

/// Packs a real `FILETIME`'s two dwords into a single raw 64-bit tick count, matching the real
/// `SUB`/`SBB`-pair arithmetic `timeAgo`/`hoursAgo` do over the two halves directly.
pub fn filetime_to_ticks(file_time: FILETIME) -> u64 {
    ((file_time.dwHighDateTime as u64) << 32) | file_time.dwLowDateTime as u64
}

/// Inverse of [`filetime_to_ticks`].
pub fn ticks_to_filetime(ticks: u64) -> FILETIME {
    FILETIME {
        dwLowDateTime: ticks as u32,
        dwHighDateTime: (ticks >> 32) as u32,
    }
}
