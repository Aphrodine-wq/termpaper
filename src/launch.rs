//! `termpaper wall up`: one terminal per monitor, sized to line up.
//!
//! Each monitor gets a kitty window running termpaper in the same link group,
//! placed with a Hyprland exec rule. Font sizes are scaled by each monitor's
//! pixel pitch so terminal cells come out the same physical size everywhere —
//! the art is equally sharp on every screen and Classic scenes resample by
//! ~1. Padding is forced to zero so the cell grid starts at the window edge.
//! Windows are classed `termpaper-wallpaper-<OUTPUT>`, which the hyprwinwrap
//! plugin (if configured for that pattern) moves into the background layer.
use crate::desk::Desk;

pub const CLASS_PREFIX: &str = "termpaper-wallpaper-";

#[derive(Clone, Debug, PartialEq)]
pub struct LaunchPlan {
    pub monitor: String,
    pub class: String,
    pub font_pt: f32,
    /// the command passed to `hyprctl dispatch exec`
    pub command: String,
}

fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./=:,@%+".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

/// Plan a window per monitor. `base_pt` is the font size on the reference
/// monitor (the first landscape one unless `reference` names another);
/// `only` restricts to some outputs; `extra` is passed to termpaper.
pub fn plan_wall_up(
    desk: &Desk,
    termpaper: &str,
    base_pt: f32,
    reference: Option<&str>,
    group: &str,
    only: &[String],
    extra: &[String],
) -> Vec<LaunchPlan> {
    let reference = reference
        .and_then(|r| desk.get(r))
        .or_else(|| desk.monitors.iter().find(|d| !d.mon.portrait()))
        .or(desk.monitors.first());
    let Some(reference) = reference else { return Vec::new() };
    let ref_pitch = reference.mm_per_px;
    desk.monitors
        .iter()
        .filter(|d| only.is_empty() || only.iter().any(|o| *o == d.mon.name))
        .map(|d| {
            // equal physical cells: point size inversely proportional to pitch
            let pt = ((base_pt as f64) * ref_pitch / d.mm_per_px * 4.0).round() / 4.0;
            let class = format!("{CLASS_PREFIX}{}", d.mon.name);
            let mut term = vec![shell_quote(termpaper), "--group".into(), shell_quote(group)];
            term.extend(extra.iter().map(|e| shell_quote(e)));
            let command = format!(
                "[monitor {}] kitty --class {} -o font_size={} -o window_padding_width=0 \
                 -o placement_strategy=top-left -o repaint_delay=5 -o input_delay=0 \
                 -o sync_to_monitor=yes -o confirm_os_window_close=0 -e {}",
                d.mon.name,
                class,
                pt,
                term.join(" ")
            );
            LaunchPlan { monitor: d.mon.name.clone(), class, font_pt: pt as f32, command }
        })
        .collect()
}

/// Spawn the planned windows, skipping monitors that already have one.
/// Returns the plans actually launched.
pub fn run_wall_up(plans: &[LaunchPlan], dry_run: bool) -> std::io::Result<Vec<LaunchPlan>> {
    let existing: Vec<String> = crate::hypr::clients()
        .unwrap_or_default()
        .into_iter()
        .map(|c| c.class)
        .collect();
    let mut launched = Vec::new();
    for p in plans {
        if existing.iter().any(|c| *c == p.class) {
            continue;
        }
        if !dry_run {
            let st = std::process::Command::new("hyprctl")
                .args(["dispatch", "exec", &p.command])
                .stdout(std::process::Stdio::null())
                .status()?;
            if !st.success() {
                return Err(std::io::Error::other(format!("hyprctl dispatch exec failed for {}", p.monitor)));
            }
        }
        launched.push(p.clone());
    }
    Ok(launched)
}

/// Close every wall window (`termpaper wall down`).
pub fn wall_down() -> std::io::Result<usize> {
    let mut n = 0;
    for c in crate::hypr::clients().unwrap_or_default() {
        if c.class.starts_with(CLASS_PREFIX) && c.pid > 0 {
            let _ = std::process::Command::new("kill").arg(c.pid.to_string()).status();
            n += 1;
        }
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::desk::{Desk, DeskConfig};
    use crate::hypr::HyprMonitor;

    fn desk() -> Desk {
        let m = |id: i64, name: &str, x: i32, t: u8, mm: (i32, i32)| HyprMonitor {
            id,
            name: name.into(),
            description: String::new(),
            width: 1920,
            height: 1080,
            refresh_rate: 60.0,
            x,
            y: 0,
            scale: 1.0,
            transform: t,
            physical_width: mm.0,
            physical_height: mm.1,
            reserved: [0, 30, 0, 0],
        };
        Desk::from_hypr(
            &[m(1, "DP-3", 0, 0, (540, 300)), m(2, "HDMI-A-1", 1920, 0, (480, 270)), m(0, "DP-1", 3840, 1, (600, 340))],
            &DeskConfig::default(),
        )
    }

    #[test]
    fn fonts_scale_so_cells_are_physically_equal() {
        let plans = plan_wall_up(&desk(), "/usr/bin/termpaper", 11.0, None, "wallpaper", &[], &[]);
        let pt = |n: &str| plans.iter().find(|p| p.monitor == n).unwrap().font_pt;
        assert_eq!(pt("DP-3"), 11.0, "the first landscape monitor is the reference");
        assert!((pt("HDMI-A-1") - 12.25).abs() < 0.01, "smaller pixels, bigger font: {}", pt("HDMI-A-1"));
        assert!((pt("DP-1") - 9.75).abs() < 0.01, "bigger pixels, smaller font: {}", pt("DP-1"));
    }

    #[test]
    fn commands_place_each_window_and_quote_arguments() {
        let plans = plan_wall_up(&desk(), "/home/me/my bin/termpaper", 11.0, None, "wallpaper", &["DP-1".into()], &["--fps".into(), "60".into()]);
        assert_eq!(plans.len(), 1);
        let c = &plans[0].command;
        assert!(c.starts_with("[monitor DP-1] kitty --class termpaper-wallpaper-DP-1"), "{c}");
        assert!(c.contains("window_padding_width=0"));
        assert!(c.ends_with("-e '/home/me/my bin/termpaper' --group wallpaper --fps 60"), "{c}");
    }
}
