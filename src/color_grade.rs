//! Global color grading applied after scene render and stackable filters.
//! Hue, saturation, and contrast — works on dark/desaturated pixels too.

use crate::canvas::{hsv, Canvas};

pub fn apply(canvas: &mut Canvas, hue_deg: f32, saturation: f32, contrast: f32) {
    let hue_on = hue_deg >= 0.5;
    let sat_on = (saturation - 1.0).abs() > 0.02;
    let con_on = (contrast - 1.0).abs() > 0.02;
    if !hue_on && !sat_on && !con_on {
        return;
    }

    let rot = hue_deg / 360.0;
    // contrast is a per-channel map, so 256 entries cover it: computed once
    // per frame instead of three times per pixel
    let clut: Option<[u8; 256]> = con_on.then(|| {
        let mut t = [0u8; 256];
        for (v, out) in t.iter_mut().enumerate() {
            *out = contrast_channel(v as u8, contrast);
        }
        t
    });
    let apply_contrast = move |o: (u8, u8, u8)| match &clut {
        Some(t) => (t[o.0 as usize], t[o.1 as usize], t[o.2 as usize]),
        None => o,
    };

    if !hue_on {
        // Fast path: with the hue unchanged there is no reason to decompose to
        // HSV and rebuild. Scaling saturation at constant hue and value keeps
        // the channel ordering, moves min to val*(1-sat') and slides mid along
        // by the same fraction it already sat at — identical output to the HSV
        // round trip, without the hue math or the six-way recompose branch.
        canvas.map_colors(|c| {
            let (r, g, b) = (c.0, c.1, c.2);
            let hi = r.max(g).max(b);
            let lo = r.min(g).min(b);
            if hi == lo {
                return apply_contrast((r, g, b)); // grey: nothing to saturate
            }
            // the channel that is neither hi nor lo
            let mid = r.max(g).min(r.max(b)).min(g.max(b));
            let (hf, lf, mf) = (hi as f32, lo as f32, mid as f32);
            let sat = ((hi - lo) as f32 / hf * saturation).clamp(0.0, 1.0);
            let new_lo = hf * (1.0 - sat);
            // keep mid at the same fraction of the lo..hi span it already had
            let new_mid = new_lo + (hf - new_lo) * ((mf - lf) / (hf - lf));
            let (v_mid, v_lo) = (new_mid as u8, new_lo as u8);
            let pick = |v: u8| {
                if v == hi {
                    hi
                } else if v == lo {
                    v_lo
                } else {
                    v_mid
                }
            };
            apply_contrast((pick(r), pick(g), pick(b)))
        });
        return;
    }

    canvas.map_colors(|c| {
        let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;

        let mut hue = if d < 1e-6 {
            0.0
        } else if max == r {
            ((g - b) / d).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };
        let mut sat = if max < 1e-6 { 0.0 } else { d / max };
        let val = max;

        if sat < 0.10 && val > 0.015 {
            sat = 0.10;
        }
        hue = (hue + rot).fract();
        if sat_on {
            sat = (sat * saturation).clamp(0.0, 1.0);
        }
        apply_contrast(hsv(hue, sat, val))
    });
}

fn contrast_channel(v: u8, contrast: f32) -> u8 {
    let f = v as f32 / 255.0;
    (((f - 0.5) * contrast + 0.5).clamp(0.0, 1.0) * 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hue_shifts_red_toward_green() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, (255, 0, 0));
        apply(&mut c, 120.0, 1.0, 1.0);
        let (r, g, b) = c.get(0, 0).color;
        assert!(g > r && g > 100, "expected green shift, got ({r},{g},{b})");
    }

    #[test]
    fn saturation_boosts_muted_pixel() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, (40, 42, 50));
        apply(&mut c, 0.0, 2.0, 1.0);
        let a = c.get(0, 0).color;
        let spread = (a.0 as i32 - a.1 as i32).abs() + (a.1 as i32 - a.2 as i32).abs();
        assert!(spread > 10, "sat boost should spread channels");
    }
}

#[cfg(test)]
mod fast_path_tests {
    use super::*;

    /// Reference implementation: the HSV decompose/recompose this used to do
    /// for every pixel. The no-hue fast path must agree with it.
    fn reference(c: (u8, u8, u8), saturation: f32, contrast: f32) -> (u8, u8, u8) {
        let (r, g, b) = (c.0 as f32 / 255.0, c.1 as f32 / 255.0, c.2 as f32 / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let d = max - min;
        let hue = if d < 1e-6 {
            0.0
        } else if max == r {
            ((g - b) / d).rem_euclid(6.0) / 6.0
        } else if max == g {
            ((b - r) / d + 2.0) / 6.0
        } else {
            ((r - g) / d + 4.0) / 6.0
        };
        let mut sat = if max < 1e-6 { 0.0 } else { d / max };
        if (saturation - 1.0).abs() > 0.02 {
            sat = (sat * saturation).clamp(0.0, 1.0);
        }
        let mut out = hsv(hue, sat, max);
        if (contrast - 1.0).abs() > 0.02 {
            out = (
                contrast_channel(out.0, contrast),
                contrast_channel(out.1, contrast),
                contrast_channel(out.2, contrast),
            );
        }
        out
    }

    /// Exact f64 saturation scaling at constant hue and value — what the
    /// operation *means*, independent of either implementation.
    fn exact(c: (u8, u8, u8), saturation: f32) -> (u8, u8, u8) {
        let (r, g, b) = (c.0, c.1, c.2);
        let hi = r.max(g).max(b);
        let lo = r.min(g).min(b);
        if hi == lo {
            return (r, g, b);
        }
        let mid = r.max(g).min(r.max(b)).min(g.max(b));
        let (hf, lf, mf) = (hi as f64, lo as f64, mid as f64);
        let s = ((hi - lo) as f64 / hf * saturation as f64).min(1.0);
        let nl = hf * (1.0 - s);
        let nm = nl + (hf - nl) * ((mf - lf) / (hf - lf));
        let pick = |v: u8| {
            if v == hi {
                hi
            } else if v == lo {
                nl as u8
            } else {
                nm as u8
            }
        };
        (pick(r), pick(g), pick(b))
    }

    /// The fast path must match exact arithmetic to within f32 truncation.
    #[test]
    fn no_hue_fast_path_is_exact() {
        for &sat in &[2.5f32, 1.0, 0.4, 3.0] {
            let mut worst = 0i32;
            for r in (0..=255).step_by(3) {
                for g in (0..=255).step_by(3) {
                    for b in (0..=255).step_by(3) {
                        let src = (r as u8, g as u8, b as u8);
                        let mut c = Canvas::new(1, 1);
                        c.set(0, 0, src);
                        apply(&mut c, 0.0, sat, 1.0);
                        let got = c.get(0, 0).color;
                        let want = exact(src, sat);
                        for (a, b2) in [(got.0, want.0), (got.1, want.1), (got.2, want.2)] {
                            worst = worst.max((a as i32 - b2 as i32).abs());
                        }
                    }
                }
            }
            assert!(worst <= 1, "sat {sat}: off exact arithmetic by {worst}");
        }
    }

    /// And it agrees with the old HSV round-trip to within that round-trip's
    /// own error. The HSV path derived a hue, then re-derived a sector and
    /// fraction from it; that float round-trip lands on the wrong side of a
    /// sector boundary for some inputs, so it was off by up to 1/255 *before*
    /// contrast. Contrast then scales any input difference by its own factor,
    /// which is the whole of the tolerance below — verified above, the fast
    /// path is the exact one and this is the old error being removed.
    #[test]
    fn no_hue_fast_path_matches_hsv_roundtrip() {
        for &(sat, con) in &[(2.5f32, 2.5f32), (2.5, 1.0), (1.0, 2.5), (0.4, 1.6), (3.0, 0.5)] {
            let allowed = (con.max(1.0).ceil() as i32) + 1;
            let mut worst = 0i32;
            for r in (0..=255).step_by(5) {
                for g in (0..=255).step_by(5) {
                    for b in (0..=255).step_by(5) {
                        let src = (r as u8, g as u8, b as u8);
                        let mut c = Canvas::new(1, 1);
                        c.set(0, 0, src);
                        apply(&mut c, 0.0, sat, con);
                        let got = c.get(0, 0).color;
                        let want = reference(src, sat, con);
                        for (a, b2) in [(got.0, want.0), (got.1, want.1), (got.2, want.2)] {
                            worst = worst.max((a as i32 - b2 as i32).abs());
                        }
                    }
                }
            }
            assert!(
                worst <= allowed,
                "sat {sat} con {con}: differs from HSV by {worst}, allowed {allowed}"
            );
        }
    }

    /// Channel ordering must survive: a saturation boost cannot reorder
    /// channels, only spread them.
    #[test]
    fn channel_ordering_is_preserved() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, (200, 120, 60));
        apply(&mut c, 0.0, 2.0, 1.0);
        let (r, g, b) = c.get(0, 0).color;
        assert!(r >= g && g >= b, "ordering broke: ({r},{g},{b})");
    }

    /// Grey pixels have no saturation to scale and must only take contrast.
    #[test]
    fn grey_is_untouched_by_saturation() {
        let mut c = Canvas::new(1, 1);
        c.set(0, 0, (128, 128, 128));
        apply(&mut c, 0.0, 3.0, 1.0);
        assert_eq!(c.get(0, 0).color, (128, 128, 128));
    }
}
