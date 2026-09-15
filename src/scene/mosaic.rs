//! Mosaic: a Voronoi stained-glass window on black. Jittered seed points
//! (12-25) partition the canvas into jewel-toned panes that breathe slowly,
//! separated by bright leaded edges (second-nearest-seed distance). Every
//! ~18-28s a flash ripple ignites one pane and spreads cell-to-cell like a
//! wave (anticipation -> payoff -> decay); the lead glints where two flashed
//! panes meet and the backlights swell behind the glass. Between waves the
//! sun catches a single pane in a brief glint. Depth: dim backlights drift
//! behind the glass, dust motes bob in front.

use super::{Detail, Scene};
use crate::canvas::{density_for, ease_smooth, glow, lerp, scale, Canvas};
use rand::{rngs::StdRng, RngExt};

// flash envelope segments (seconds): slow anticipation glow, a fast snap to
// full flare, then a long decay
const ANT: f32 = 0.9;
const PAY: f32 = 0.15;
// decay is shorter than the wave's crossing time so the flare reads as a
// moving front with a fading tail, not the whole window lighting at once
const DEC: f32 = 1.4;
const FLASH_TOTAL: f32 = ANT + PAY + DEC;
/// glint micro-event: one pane catches the sun for a moment
const GLINT_TOTAL: f32 = 1.6;

/// One Voronoi seed: a glass pane.
struct Seed {
    x: f32,
    y: f32,
    tint: (u8, u8, u8),
    phase: f32, // breathing phase
    speed: f32, // breathing rate
    val: f32,   // per-pane value jitter so neighboring panes stay distinct
}

/// Dim light drifting behind the glass (far depth plane).
struct Backlight {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    r2: f32, // squared influence radius
    color: (u8, u8, u8),
}

/// Foreground dust mote (near depth plane).
struct Mote {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    phase: f32,
}

/// The flash ripple: one pane ignites, the wave spreads outward pane by pane.
struct Wave {
    origin: usize,
    elapsed: f32,
    max_delay: f32, // ignition delay of the farthest pane
}

/// Flash envelope 0..1 for a pane `x` seconds after its own ignition:
/// anticipation ramp, payoff snap, long decay.
fn flash_env(x: f32) -> f32 {
    if x < 0.0 || x > FLASH_TOTAL {
        0.0
    } else if x < ANT {
        ease_smooth(x / ANT) * 0.35
    } else if x < ANT + PAY {
        0.35 + 0.65 * ease_smooth((x - ANT) / PAY)
    } else {
        1.0 - ease_smooth((x - ANT - PAY) / DEC)
    }
}

pub struct Mosaic {
    rng: StdRng,
    detail: Detail,
    palette: Vec<(u8, u8, u8)>,
    lead: (u8, u8, u8),
    seeds: Vec<Seed>,
    blobs: Vec<Backlight>,
    motes: Vec<Mote>,
    flash: Vec<f32>,   // scratch: per-pane flare this frame
    breathe: Vec<f32>, // scratch: per-pane breathing brightness
    wave: Option<Wave>,
    next_flash: f32,
    /// micro-event between waves: sun catches one pane, it glints and fades
    glint: Option<(usize, f32)>, // (pane, elapsed)
    next_glint: f32,
    spacing: f32, // typical pane size in pixels
    t: f32,
    w: usize,
    h: usize,
}

impl Mosaic {
    pub fn new(rng: StdRng, theme: Option<&str>, detail: Detail) -> Self {
        let (palette, lead) = match theme {
            Some("ocean") => (
                vec![
                    (18, 86, 158),
                    (22, 150, 168),
                    (58, 198, 188),
                    (14, 56, 118),
                    (96, 214, 224),
                ],
                (168, 214, 228),
            ),
            Some("mono") => (
                vec![
                    (86, 86, 96),
                    (138, 138, 148),
                    (182, 182, 192),
                    (62, 62, 72),
                    (214, 214, 220),
                ],
                (198, 198, 206),
            ),
            // cathedral: jewel tones — ruby, sapphire, amber, emerald, amethyst
            _ => (
                vec![
                    (196, 32, 58),
                    (44, 84, 216),
                    (226, 158, 34),
                    (30, 172, 92),
                    (148, 62, 198),
                ],
                (216, 190, 150),
            ),
        };
        Mosaic {
            rng,
            detail,
            palette,
            lead,
            seeds: Vec::new(),
            blobs: Vec::new(),
            motes: Vec::new(),
            flash: Vec::new(),
            breathe: Vec::new(),
            wave: None,
            next_flash: 9.0, // first ripple lands early; later ones 18-28s
            glint: None,
            next_glint: 3.5, // first glint lands early; later ones 4-9s
            spacing: 1.0,
            t: 0.0,
            w: 0,
            h: 0,
        }
    }

    /// (Re)build the pane layout for a new canvas size.
    fn init(&mut self, w: usize, h: usize) {
        let n = self
            .detail
            .scale(16.0 * density_for(w, h), 12)
            .min(25);
        // jittered grid: cols follow the canvas aspect so panes stay squarish
        let cols = ((n as f32 * w as f32 / h as f32).sqrt().round() as usize).max(1);
        let rows = n.div_ceil(cols).max(1);
        let cw = w as f32 / cols as f32;
        let ch = h as f32 / rows as f32;
        let palette = self.palette.clone();
        self.seeds = (0..n)
            .map(|k| {
                let (c, r) = (k % cols, k / cols);
                let x = (c as f32 + 0.5 + self.rng.random_range(-0.32..0.32)) * cw;
                let y = (r as f32 + 0.5 + self.rng.random_range(-0.32..0.32)) * ch;
                Seed {
                    x: x.clamp(0.0, w as f32 - 1.0),
                    y: y.clamp(0.0, h as f32 - 1.0),
                    tint: palette[self.rng.random_range(0..palette.len())],
                    phase: self.rng.random_range(0.0..std::f32::consts::TAU),
                    speed: self.rng.random_range(0.22..0.5),
                    val: self.rng.random_range(0.6..1.3),
                }
            })
            .collect();
        self.spacing = ((w * h) as f32 / n as f32).sqrt();
        // far plane: a few dim lights drifting behind the glass
        let r = self.spacing * 1.8;
        self.blobs = (0..3)
            .map(|i| Backlight {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                vx: self.rng.random_range(-1.6..1.6),
                vy: self.rng.random_range(-1.0..1.0),
                r2: r * r,
                color: palette[i % palette.len()],
            })
            .collect();
        // near plane: sparse dust motes glinting in front of the window
        let nm = self
            .detail
            .scale(6.0 * density_for(w, h), 3)
            .min(12);
        self.motes = (0..nm)
            .map(|_| Mote {
                x: self.rng.random_range(0.0..w as f32),
                y: self.rng.random_range(0.0..h as f32),
                vx: self.rng.random_range(1.5..5.0)
                    * if self.rng.random::<bool>() { 1.0 } else { -1.0 },
                vy: self.rng.random_range(-0.8..0.8),
                phase: self.rng.random_range(0.0..std::f32::consts::TAU),
            })
            .collect();
        self.flash = vec![0.0; n];
        self.breathe = vec![0.0; n];
        self.wave = None;
        self.glint = None;
    }
}

impl Scene for Mosaic {
    fn name(&self) -> &'static str {
        "mosaic"
    }

    fn update(&mut self, dt: f32, canvas: &mut Canvas) {
        let (w, h) = (canvas.width(), canvas.height());
        if w == 0 || h == 0 {
            return;
        }
        // survive fast-forward: cap per-step effects
        let dt = dt.clamp(0.0, 0.1);

        if w != self.w || h != self.h {
            self.w = w;
            self.h = h;
            self.init(w, h);
        }
        self.t += dt;
        let n = self.seeds.len();

        // --- per-pane breathing: slow eased sine, dim glass body; the per-pane
        // value jitter keeps same-tint neighbors from merging into one blob ---
        for (i, s) in self.seeds.iter().enumerate() {
            self.breathe[i] =
                (0.05 + 0.09 * (0.5 + 0.5 * (self.t * s.speed + s.phase).sin())) * s.val;
        }

        // --- glint: between waves, the sun catches a single pane (eased
        // in/out, no popping) and its lead rim warms with it ---
        self.next_glint -= dt;
        if self.glint.is_none() && self.next_glint <= 0.0 {
            self.glint = Some((self.rng.random_range(0..n), 0.0));
        }
        if let Some((_, elapsed)) = &mut self.glint {
            *elapsed += dt;
            if *elapsed > GLINT_TOTAL {
                self.glint = None;
                self.next_glint = self.rng.random_range(4.0..9.0);
            }
        }

        // --- flash ripple scheduling (anticipation -> payoff -> decay) ---
        let diag = (w as f32 * w as f32 + h as f32 * h as f32).sqrt();
        let wave_speed = diag / 2.6; // the wave crosses the window in ~2.6s
        self.next_flash -= dt;
        if self.wave.is_none() && self.next_flash <= 0.0 {
            let origin = self.rng.random_range(0..n);
            let (ox, oy) = (self.seeds[origin].x, self.seeds[origin].y);
            let max_d = self
                .seeds
                .iter()
                .map(|s| ((s.x - ox).powi(2) + (s.y - oy).powi(2)).sqrt())
                .fold(0.0f32, f32::max);
            self.wave = Some(Wave {
                origin,
                elapsed: 0.0,
                max_delay: max_d / wave_speed,
            });
        }
        if let Some(wave) = &mut self.wave {
            wave.elapsed += dt;
            if wave.elapsed > wave.max_delay + FLASH_TOTAL {
                self.wave = None;
                self.next_flash = self.rng.random_range(18.0..28.0);
            }
        }
        // per-pane flare: envelope delayed by distance from the origin pane,
        // amplitude fading as the wave spends itself
        for i in 0..n {
            self.flash[i] = match &self.wave {
                Some(wave) => {
                    let (ox, oy) = (self.seeds[wave.origin].x, self.seeds[wave.origin].y);
                    let d = ((self.seeds[i].x - ox).powi(2) + (self.seeds[i].y - oy).powi(2)).sqrt();
                    let amp = 1.0 - 0.55 * (d / (wave.max_delay * wave_speed).max(1.0)).min(1.0);
                    flash_env(wave.elapsed - d / wave_speed) * amp
                }
                None => 0.0,
            };
        }
        // fold the glint into its pane's flare so the body, the lead rim and
        // the neighbor spill all catch the light together
        if let Some((pane, elapsed)) = self.glint {
            let e = elapsed / GLINT_TOTAL;
            let k = if e < 0.4 {
                ease_smooth(e / 0.4)
            } else {
                1.0 - ease_smooth((e - 0.4) / 0.6)
            };
            self.flash[pane] = (self.flash[pane] + k * 0.32).min(1.2);
        }
        // wave energy: the whole window backlights while a flare runs
        let wave_glow = self.flash.iter().cloned().fold(0.0f32, f32::max);

        // --- drift the depth planes ---
        for b in &mut self.blobs {
            b.x = (b.x + b.vx * dt).rem_euclid(w as f32);
            b.y = (b.y + b.vy * dt).rem_euclid(h as f32);
        }
        for m in &mut self.motes {
            m.x = (m.x + m.vx * dt).rem_euclid(w as f32);
            m.y = (m.y + m.vy * dt).rem_euclid(h as f32);
        }

        // --- draw ---
        canvas.clear((0, 0, 0));
        let edge_w = (self.spacing * 0.055).clamp(0.7, 1.8);
        let spill_w = edge_w * 3.0;
        // the whole window lifts while any pane is flaring, so a flare reads as
        // light entering the room rather than as one pane switching on alone
        let ambient = wave_glow * 0.12;

        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (px, py) = (x as f32, y as f32);
                // nearest + second-nearest seed (n <= 25: brute force is cheap)
                let (mut i1, mut i2) = (0usize, 0usize);
                let (mut d1, mut d2) = (f32::MAX, f32::MAX);
                for (i, s) in self.seeds.iter().enumerate() {
                    let dd = (s.x - px).powi(2) + (s.y - py).powi(2);
                    if dd < d1 {
                        d2 = d1;
                        i2 = i1;
                        d1 = dd;
                        i1 = i;
                    } else if dd < d2 {
                        d2 = dd;
                        i2 = i;
                    }
                }
                let gap = d2.sqrt() - d1.sqrt();
                let (f1, f2) = (self.flash[i1], self.flash[i2]);

                if gap < edge_w {
                    // lead came: bright edge; glints hot where two flashed
                    // panes meet, warms softly when either side flares
                    let meet = (f1 * f2 * 1.1).min(1.0);
                    let bright = (0.30 + (f1 + f2) * 0.15 + meet * 0.9 + ambient * 0.6).min(1.2);
                    let c = scale(lerp(self.lead, (255, 255, 255), meet * 0.5), bright);
                    canvas.set(x, y, c);
                } else {
                    // glass body: dim breathing tint, brighter toward the pane
                    // center, plus flare and a neighbor's light spilling over
                    let shade = 1.0 - 0.25 * (d1.sqrt() / self.spacing).min(1.0);
                    let spill = if gap < spill_w {
                        f2 * (1.0 - gap / spill_w) * 0.3
                    } else {
                        0.0
                    };
                    let flare = (f1 + spill).min(1.2);
                    let tint = self.seeds[i1].tint;
                    let hot = lerp(tint, (255, 255, 255), 0.45);
                    let lit = flare * 0.85 + ambient;
                    let (mut r, mut g, mut b) = (
                        (tint.0 as f32 * self.breathe[i1] * shade + hot.0 as f32 * lit).min(255.0),
                        (tint.1 as f32 * self.breathe[i1] * shade + hot.1 as f32 * lit).min(255.0),
                        (tint.2 as f32 * self.breathe[i1] * shade + hot.2 as f32 * lit).min(255.0),
                    );
                    // far plane: backlights bleeding dimly through the glass
                    for bl in &self.blobs {
                        let dd = (bl.x - px).powi(2) + (bl.y - py).powi(2);
                        let f = bl.r2 / (bl.r2 + dd) * 0.06;
                        r = (r + bl.color.0 as f32 * f).min(255.0);
                        g = (g + bl.color.1 as f32 * f).min(255.0);
                        b = (b + bl.color.2 as f32 * f).min(255.0);
                    }
                    canvas.set(x, y, (r as u8, g as u8, b as u8));
                }
            }
        }

        // payoff pop: flaring panes cast a bounded glow around their hearts
        for (i, s) in self.seeds.iter().enumerate() {
            let f = self.flash[i];
            if f > 0.55 {
                let r = (2.0 + f * 2.0).min(4.0) as i32;
                let hot = lerp(s.tint, (255, 255, 255), 0.45);
                glow(canvas, s.x as i32, s.y as i32, r, hot, f * 0.4);
            }
        }

        // near plane: dust motes twinkling as they drift across the window
        for m in &self.motes {
            let tw = 0.5 + 0.5 * (self.t * 1.7 + m.phase).sin();
            if tw > 0.72 {
                let b = (tw - 0.72) / 0.28;
                canvas.add(m.x as i32, m.y as i32, scale((205, 205, 215), b * b * 0.45));
            }
        }
    }
}
