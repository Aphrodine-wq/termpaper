//! What differs between Linux, macOS and Windows, in one place: where files
//! live, whether a pid is alive, the local time, the power source, and the
//! Windows timer resolution. Everything else asks here instead of reading
//! environment variables or `/proc` itself.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

fn env_dir(key: &str) -> Option<PathBuf> {
    std::env::var_os(key).filter(|v| !v.is_empty()).map(PathBuf::from)
}

/// The user's home directory.
pub fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        env_dir("USERPROFILE").or_else(|| env_dir("HOME"))
    }
    #[cfg(not(windows))]
    {
        env_dir("HOME")
    }
}

/// termpaper's config directory: `$XDG_CONFIG_HOME/termpaper`, else
/// `%APPDATA%\termpaper` on Windows, else `~/.config/termpaper` (Linux and
/// macOS alike: terminal tools on macOS conventionally live there too).
pub fn config_dir() -> Option<PathBuf> {
    if let Some(x) = env_dir("XDG_CONFIG_HOME") {
        return Some(x.join("termpaper"));
    }
    #[cfg(windows)]
    if let Some(a) = env_dir("APPDATA") {
        return Some(a.join("termpaper"));
    }
    home_dir().map(|h| h.join(".config").join("termpaper"))
}

/// Logs and other state: `$XDG_STATE_HOME/termpaper`, else
/// `%LOCALAPPDATA%\termpaper` on Windows, else `~/.local/state/termpaper`.
pub fn state_dir() -> Option<PathBuf> {
    if let Some(x) = env_dir("XDG_STATE_HOME") {
        return Some(x.join("termpaper"));
    }
    #[cfg(windows)]
    if let Some(a) = env_dir("LOCALAPPDATA") {
        return Some(a.join("termpaper"));
    }
    home_dir().map(|h| h.join(".local").join("state").join("termpaper"))
}

/// Base of the live-instance registry: `$XDG_RUNTIME_DIR/termpaper`, else
/// `%TEMP%\termpaper` on Windows (already per user), else
/// `/tmp/termpaper-$UID`.
pub fn runtime_dir() -> Option<PathBuf> {
    if let Some(x) = env_dir("XDG_RUNTIME_DIR") {
        return Some(x.join("termpaper"));
    }
    #[cfg(windows)]
    {
        Some(std::env::temp_dir().join("termpaper"))
    }
    #[cfg(not(windows))]
    {
        Some(PathBuf::from(format!("/tmp/termpaper-{}", uid())))
    }
}

#[cfg(unix)]
fn uid() -> u32 {
    // SAFETY: getuid has no preconditions and cannot fail
    unsafe { libc::getuid() }
}

/// Whether a process with this pid exists. A process owned by someone else
/// still counts: it is alive, just not ours to signal.
#[cfg(unix)]
pub fn pid_alive(pid: u32) -> bool {
    // pid 0 addresses our own process group, and anything past i32 is not a
    // pid at all
    if pid == 0 || pid > i32::MAX as u32 {
        return false;
    }
    // kill(pid, 0): 0 = alive and ours, EPERM = alive but not ours, ESRCH = gone
    // SAFETY: signal 0 only checks for existence and permission
    if unsafe { libc::kill(pid as libc::pid_t, 0) } == 0 {
        return true;
    }
    std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

/// Whether a process with this pid exists. A process owned by someone else
/// still counts: it is alive, just not ours to open.
#[cfg(windows)]
pub fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ACCESS_DENIED, STILL_ACTIVE};
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    if pid == 0 {
        return false;
    }
    // SAFETY: plain Win32 calls; the handle is closed on every path
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let ok = GetExitCodeProcess(h, &mut code) != 0;
        CloseHandle(h);
        ok && code == STILL_ACTIVE as u32
    }
}

/// Local wall-clock time, time zone and daylight saving applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalTime {
    pub year: i32,
    /// 1..=12
    pub month: u32,
    /// 1..=31
    pub day: u32,
    /// 0 = Monday .. 6 = Sunday
    pub weekday: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}

impl LocalTime {
    /// Hours since midnight as a fraction (13:30 → 13.5).
    pub fn hours(&self) -> f32 {
        self.hour as f32 + self.minute as f32 / 60.0 + self.second as f32 / 3600.0
    }
}

pub fn local_time() -> LocalTime {
    use chrono::{Datelike, Timelike};
    let t = chrono::Local::now();
    LocalTime {
        year: t.year(),
        month: t.month(),
        day: t.day(),
        weekday: t.weekday().num_days_from_monday(),
        hour: t.hour(),
        minute: t.minute(),
        second: t.second(),
    }
}

/// Where the machine draws power from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Power {
    /// mains power, or a machine without a battery
    Ac,
    Battery,
    Unknown,
}

impl Power {
    fn to_u8(self) -> u8 {
        match self {
            Power::Ac => 0,
            Power::Battery => 1,
            Power::Unknown => 2,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => Power::Ac,
            1 => Power::Battery,
            _ => Power::Unknown,
        }
    }
}

/// Parse `/sys/class/power_supply` entries given as (type, online, status)
/// triples. Any battery that is discharging means battery power; a machine
/// with no battery is on mains.
pub fn power_from_supplies(supplies: &[(String, Option<String>, Option<String>)]) -> Power {
    let mut any_battery = false;
    let mut mains_online = false;
    let mut discharging = false;
    for (kind, online, status) in supplies {
        match kind.trim() {
            "Battery" => {
                any_battery = true;
                if status.as_deref().map(str::trim) == Some("Discharging") {
                    discharging = true;
                }
            }
            "Mains" | "USB" | "USB_C" | "USB_PD" => {
                if online.as_deref().map(str::trim) == Some("1") {
                    mains_online = true;
                }
            }
            _ => {}
        }
    }
    if !any_battery || mains_online {
        Power::Ac
    } else if discharging {
        Power::Battery
    } else {
        Power::Ac
    }
}

/// Parse the first line of `pmset -g batt`: "Now drawing from 'AC Power'".
pub fn power_from_pmset(text: &str) -> Power {
    let first = text.lines().next().unwrap_or("");
    if first.contains("'Battery Power'") {
        Power::Battery
    } else if first.contains("'AC Power'") || first.contains("'UPS Power'") {
        Power::Ac
    } else {
        Power::Unknown
    }
}

/// Ask the OS once where power comes from. May spawn `pmset` on macOS, so
/// the frame loop reads a [`PowerWatcher`] instead.
pub fn power_source() -> Power {
    #[cfg(target_os = "linux")]
    {
        let Ok(rd) = std::fs::read_dir("/sys/class/power_supply") else {
            return Power::Unknown;
        };
        let read = |p: &std::path::Path, f: &str| std::fs::read_to_string(p.join(f)).ok();
        let supplies: Vec<_> = rd
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                Some((read(&p, "type")?, read(&p, "online"), read(&p, "status")))
            })
            .collect();
        power_from_supplies(&supplies)
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("pmset")
            .args(["-g", "batt"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| power_from_pmset(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or(Power::Unknown)
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        // SAFETY: the struct is plain data the call fills in
        let mut s: SYSTEM_POWER_STATUS = unsafe { std::mem::zeroed() };
        if unsafe { GetSystemPowerStatus(&mut s) } == 0 {
            return Power::Unknown;
        }
        // BatteryFlag 128: no system battery
        match (s.ACLineStatus, s.BatteryFlag) {
            (_, 128) => Power::Ac,
            (0, _) => Power::Battery,
            (1, _) => Power::Ac,
            _ => Power::Unknown,
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        Power::Unknown
    }
}

/// Polls the power source on a background thread; reading it is free.
pub struct PowerWatcher {
    state: Arc<AtomicU8>,
}

impl PowerWatcher {
    pub const PERIOD: Duration = Duration::from_secs(30);

    pub fn spawn() -> Self {
        let state = Arc::new(AtomicU8::new(Power::Unknown.to_u8()));
        let slot = Arc::clone(&state);
        let _ = std::thread::Builder::new()
            .name("termpaper-power".into())
            .spawn(move || loop {
                slot.store(power_source().to_u8(), Ordering::Relaxed);
                // the watcher is dropped with the process; a detached thread
                // sleeping between polls costs nothing
                std::thread::sleep(Self::PERIOD);
            });
        PowerWatcher { state }
    }

    pub fn get(&self) -> Power {
        Power::from_u8(self.state.load(Ordering::Relaxed))
    }
}

/// Raises the Windows timer resolution to 1 ms while alive. Without it,
/// every timed wait (the frame loop's input poll included) rounds up to the
/// 15.6 ms scheduler tick and pacing above ~60 fps falls apart. A no-op
/// elsewhere.
pub struct TimerResolution {
    _private: (),
}

impl TimerResolution {
    pub fn raise() -> Self {
        #[cfg(windows)]
        // SAFETY: balanced by timeEndPeriod in Drop
        unsafe {
            windows_sys::Win32::Media::timeBeginPeriod(1);
        }
        TimerResolution { _private: () }
    }
}

impl Drop for TimerResolution {
    fn drop(&mut self) {
        #[cfg(windows)]
        // SAFETY: matches the timeBeginPeriod in raise
        unsafe {
            windows_sys::Win32::Media::timeEndPeriod(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &str) -> String {
        v.to_string()
    }

    #[test]
    fn self_is_alive_and_nonsense_is_not() {
        assert!(pid_alive(std::process::id()));
        assert!(!pid_alive(0));
        assert!(!pid_alive(0x7FFF_FFFF));
    }

    #[test]
    fn dirs_honour_xdg_overrides() {
        // only reads the environment: each helper appends termpaper
        if let Some(d) = config_dir() {
            assert!(d.ends_with("termpaper"), "{d:?}");
        }
        if let Some(d) = state_dir() {
            assert!(d.ends_with("termpaper"), "{d:?}");
        }
        let r = runtime_dir().expect("a runtime dir everywhere");
        assert!(r.to_string_lossy().contains("termpaper"), "{r:?}");
    }

    #[test]
    fn linux_power_supplies() {
        // desktop: no battery at all
        assert_eq!(power_from_supplies(&[]), Power::Ac);
        assert_eq!(power_from_supplies(&[(s("Mains"), Some(s("0\n")), None)]), Power::Ac);
        // laptop unplugged and discharging
        let unplugged = [
            (s("Mains"), Some(s("0\n")), None),
            (s("Battery"), None, Some(s("Discharging\n"))),
        ];
        assert_eq!(power_from_supplies(&unplugged), Power::Battery);
        // plugged in (charging or full)
        let plugged = [
            (s("Mains"), Some(s("1\n")), None),
            (s("Battery"), None, Some(s("Charging\n"))),
        ];
        assert_eq!(power_from_supplies(&plugged), Power::Ac);
        let full = [(s("Battery"), None, Some(s("Full\n")))];
        assert_eq!(power_from_supplies(&full), Power::Ac);
    }

    #[test]
    fn pmset_first_line() {
        assert_eq!(
            power_from_pmset("Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1) 80%; discharging"),
            Power::Battery
        );
        assert_eq!(power_from_pmset("Now drawing from 'AC Power'\n"), Power::Ac);
        assert_eq!(power_from_pmset(""), Power::Unknown);
    }

    #[test]
    fn local_time_is_sane() {
        let t = local_time();
        assert!(t.hour < 24 && t.minute < 60 && t.second < 61);
        assert!((1..=12).contains(&t.month) && (1..=31).contains(&t.day));
        assert!(t.weekday < 7);
        assert!(t.hours() >= 0.0 && t.hours() < 24.0);
    }

    #[test]
    fn timer_guard_is_harmless() {
        let g = TimerResolution::raise();
        drop(g);
    }
}
