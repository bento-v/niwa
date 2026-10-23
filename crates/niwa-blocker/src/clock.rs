//! The session clock.

use std::time::Duration;

use windows::core::PCWSTR;
use windows::Win32::Foundation::SYSTEMTIME;
use windows::Win32::Globalization::{GetTimeFormatEx, TIME_NOSECONDS};
use windows::Win32::System::WindowsProgramming::QueryUnbiasedInterruptTime;

/// Time the PC has spent awake since boot.
///
/// Sessions are timed with this rather than the wall clock: it can't be moved
/// by changing the system time, and it stands still while the PC is asleep,
/// so time asleep doesn't count toward a session while time locked does.
pub fn awake_time() -> Duration {
    let mut ticks = 0u64;
    // SAFETY: `ticks` is a valid out pointer. The call only fails for a null pointer.
    unsafe {
        let _ = QueryUnbiasedInterruptTime(&mut ticks);
    }
    // Ticks are 100 ns.
    Duration::from_nanos(ticks * 100)
}

/// A time of day as this user's Windows shows it, without seconds: "9:00 AM"
/// or "09:00", as the taskbar clock does.
pub fn time_of_day(hour: u32, minute: u32) -> String {
    let time = SYSTEMTIME {
        wHour: hour as u16,
        wMinute: minute as u16,
        ..Default::default()
    };
    let mut text = [0u16; 64];
    // SAFETY: `time` and `text` outlive the call, which writes at most `text.len()` units.
    let written = unsafe { GetTimeFormatEx(PCWSTR::null(), TIME_NOSECONDS, Some(&time), PCWSTR::null(), Some(&mut text)) };
    match written {
        // The count includes the closing NUL.
        2.. => String::from_utf16_lossy(&text[..written as usize - 1]),
        _ => format!("{hour}:{minute:02}"),
    }
}
