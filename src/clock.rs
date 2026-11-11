//! Local calendar date, matching `datetime.date.today()` and `date.fromisoformat`.

pub fn today() -> String {
    let (y, m, d) = local_ymd();
    format!("{y:04}-{m:02}-{d:02}")
}

pub fn valid_date(value: &str) -> bool {
    let b = value.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' {
        return false;
    }
    if !b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        return false;
    }
    let y: i32 = value[0..4].parse().unwrap_or(0);
    let m: u32 = value[5..7].parse().unwrap_or(0);
    let d: u32 = value[8..10].parse().unwrap_or(0);
    if !(1..=9999).contains(&y) || !(1..=12).contains(&m) {
        return false;
    }
    d >= 1 && d <= days_in_month(y, m)
}

fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if is_leap(year) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

fn is_leap(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn local_ymd() -> (i32, u32, u32) {
    #[cfg(windows)]
    {
        windows_ymd()
    }
    #[cfg(not(windows))]
    {
        unix_ymd()
    }
}

#[cfg(windows)]
#[repr(C)]
#[derive(Default)]
struct SystemTime {
    year: u16,
    month: u16,
    dow: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    ms: u16,
}

#[cfg(windows)]
extern "system" {
    fn GetLocalTime(out: *mut SystemTime);
    fn TzSpecificLocalTimeToSystemTime(
        zone: *const std::ffi::c_void,
        local: *const SystemTime,
        universal: *mut SystemTime,
    ) -> i32;
}

#[cfg(windows)]
fn windows_ymd() -> (i32, u32, u32) {
    let mut st = SystemTime::default();
    unsafe { GetLocalTime(&mut st) }
    if st.year == 0 {
        return utc_fallback();
    }
    (st.year as i32, st.month as u32, st.day as u32)
}

#[cfg(not(windows))]
#[repr(C)]
struct Tm {
    tm_sec: i32,
    tm_min: i32,
    tm_hour: i32,
    tm_mday: i32,
    tm_mon: i32,
    tm_year: i32,
    tm_wday: i32,
    tm_yday: i32,
    tm_isdst: i32,
    // tm_gmtoff and tm_zone on glibc and macOS, with room to spare.
    pad: [i64; 8],
}

#[cfg(not(windows))]
extern "C" {
    fn time(tloc: *mut i64) -> i64;
    fn localtime_r(timep: *const i64, result: *mut Tm) -> *mut Tm;
    fn mktime(tm: *mut Tm) -> i64;
    fn tzset();
}

/// Seconds since the Unix epoch for a wall-clock time in the local time
/// zone (the one `today()` and `updated` use). Falls back to UTC if the
/// OS cannot convert it.
fn local_to_epoch(y: i32, m: u32, d: u32, h: i64, mi: i64, s: i64) -> i64 {
    let utc = days_from_civil(y, m, d) * 86_400 + h * 3600 + mi * 60 + s;
    #[cfg(windows)]
    {
        let local = SystemTime {
            year: y as u16,
            month: m as u16,
            dow: 0,
            day: d as u16,
            hour: h as u16,
            minute: mi as u16,
            second: s as u16,
            ms: 0,
        };
        let mut out = SystemTime::default();
        if unsafe { TzSpecificLocalTimeToSystemTime(std::ptr::null(), &local, &mut out) } == 0 {
            return utc;
        }
        days_from_civil(out.year as i32, out.month as u32, out.day as u32) * 86_400
            + i64::from(out.hour) * 3600
            + i64::from(out.minute) * 60
            + i64::from(out.second)
    }
    #[cfg(not(windows))]
    unsafe {
        let mut tm = std::mem::zeroed::<Tm>();
        tm.tm_year = y - 1900;
        tm.tm_mon = m as i32 - 1;
        tm.tm_mday = d as i32;
        tm.tm_hour = h as i32;
        tm.tm_min = mi as i32;
        tm.tm_sec = s as i32;
        tm.tm_isdst = -1;
        tzset();
        match mktime(&mut tm) {
            -1 => utc,
            secs => secs,
        }
    }
}

#[cfg(not(windows))]
fn unix_ymd() -> (i32, u32, u32) {
    unsafe {
        let mut secs: i64 = 0;
        if time(&mut secs) < 0 {
            return utc_fallback();
        }
        let mut tm = std::mem::zeroed::<Tm>();
        if localtime_r(&secs, &mut tm).is_null() || tm.tm_mday == 0 {
            return utc_fallback();
        }
        (tm.tm_year + 1900, (tm.tm_mon + 1) as u32, tm.tm_mday as u32)
    }
}

fn utc_fallback() -> (i32, u32, u32) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    civil_from_days(days)
}

/// Howard Hinnant's civil_from_days, days since 1970-01-01.
fn civil_from_days(z: i64) -> (i32, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

/// Howard Hinnant's days_from_civil: days since 1970-01-01.
fn days_from_civil(y: i32, m: u32, d: u32) -> i64 {
    let y = i64::from(if m <= 2 { y - 1 } else { y });
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let m = u64::from(m);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + u64::from(d) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

/// Seconds since the Unix epoch for an `ls` `since` value, `None` if it is
/// not one. A bare `YYYY-MM-DD` is local midnight: the same calendar as
/// `updated` and `today()`, so a date read off a note means that day.
/// A date-time is `YYYY-MM-DDTHH:MM[:SS[.frac]]` (`T`, `t`, or a space)
/// followed by `Z`, `+HH:MM`, `-HH:MM`, `+HHMM`, `+HH`, or nothing for local
/// time. Fractions of a second are dropped, since `since` is inclusive to
/// the second.
pub fn parse_since(text: &str) -> Option<i64> {
    let text = text.trim();
    let date = text.get(..10)?;
    if !valid_date(date) {
        return None;
    }
    let y: i32 = date[0..4].parse().ok()?;
    let m: u32 = date[5..7].parse().ok()?;
    let d: u32 = date[8..10].parse().ok()?;
    let rest = &text[10..];
    if rest.is_empty() {
        return Some(local_to_epoch(y, m, d, 0, 0, 0));
    }
    let rest = rest.strip_prefix(['T', 't', ' '])?;
    let digits2 = |field: &str| -> Option<i64> {
        (field.len() == 2 && field.bytes().all(|c| c.is_ascii_digit())).then(|| field.parse().ok())?
    };
    let h = digits2(rest.get(0..2)?)?;
    let mi = digits2(rest.get(3..5).filter(|_| rest.as_bytes()[2] == b':')?)?;
    let mut rest = &rest[5..];
    let mut s = 0;
    if let Some(tail) = rest.strip_prefix(':') {
        s = digits2(tail.get(0..2)?)?;
        rest = &tail[2..];
        if let Some(frac) = rest.strip_prefix('.') {
            let n = frac.bytes().take_while(u8::is_ascii_digit).count();
            if n == 0 {
                return None;
            }
            rest = &frac[n..];
        }
    }
    if h > 23 || mi > 59 || s > 59 {
        return None;
    }
    let offset = match rest {
        "" => return Some(local_to_epoch(y, m, d, h, mi, s)),
        "Z" | "z" => 0,
        zone => {
            let sign = match zone.as_bytes()[0] {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            // ASCII only, so the byte slices below are on char boundaries.
            let zone = &zone[1..];
            if !zone.is_ascii() {
                return None;
            }
            let (zh, zm) = match zone.len() {
                2 => (digits2(zone)?, 0),
                4 => (digits2(&zone[..2])?, digits2(&zone[2..])?),
                5 if zone.as_bytes()[2] == b':' => (digits2(&zone[..2])?, digits2(&zone[3..])?),
                _ => return None,
            };
            if zh > 23 || zm > 59 {
                return None;
            }
            sign * (zh * 3600 + zm * 60)
        }
    };
    Some(days_from_civil(y, m, d) * 86_400 + h * 3600 + mi * 60 + s - offset)
}

/// `YYYY-MM-DDTHH:MM:SSZ` for seconds since the Unix epoch.
pub fn format_utc(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let rest = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rest / 3600,
        rest % 3600 / 60,
        rest % 60
    )
}

/// Whole seconds since the Unix epoch; earlier times count as 0.
pub fn epoch_secs(time: std::time::SystemTime) -> i64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
