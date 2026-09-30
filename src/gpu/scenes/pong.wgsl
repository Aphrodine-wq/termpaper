//! name: pong
//! title: Pong
//! category: games
//! tags: pong, retro, arcade, paddles, game, 1972
//! desc: two paddles that never miss, a square ball, a dashed net and a score that keeps climbing
//! themes: classic, neon, crt
//! params: speed, trail
//! uses: game, glyph
//! cost: light
//! fallback: dvd
//! credits: original

// Pong, 1972, playing itself forever. Nothing is remembered: the ball is two
// triangle waves with incommensurate speeds, each paddle glides from where it
// met the ball last to where it will meet it next, and the score is the number
// of half cycles the ball has completed. The court is the frame: walls at
// y = ±0.42, paddles at x = ±0.68, plain background beyond.

const BALL_HALF: f32 = 0.012;
const WALL_Y: f32 = 0.42;
const WALL_HALF: f32 = 0.006;
const PADDLE_X: f32 = 0.68;
const PADDLE_H: vec2f = vec2f(0.012, 0.07);
const BX_LO: f32 = -0.66;
const BX_HI: f32 = 0.66;
const BY_LO: f32 = -0.40;
const BY_HI: f32 = 0.40;

struct Look {
    bg: vec3f,
    paddle: vec3f,
    ball: vec3f,
    net: vec3f,
    text: vec3f,
    wall: vec3f,
    glow: f32,          // halo strength around the ball (and a little on the paddles)
    trail_gain: f32,    // multiplies the trail alpha
    crt: f32,           // 1 = scanlines, vignette, bright phosphor centre
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {
            // neon: deep navy court, cyan paddles, a magenta ball that blooms
            let cyan = col_hex(0x2af0ffu);
            return Look(col_hex(0x0a0f3au), cyan * 1.3, col_hex(0xff2fd0u) * 1.7,
                        col_hex(0x6a4cffu) * 0.55, cyan * 0.9, cyan * 0.8, 1.0, 1.8, 0.0);
        }
        case 2u: {
            // crt: green phosphor on a faintly lit tube
            let ink = col_hex(0x33ff66u);
            return Look(ink * 0.028, ink, ink * 1.25, ink * 0.55, ink, ink * 0.8, 0.3, 1.2, 1.0);
        }
        default: {
            // classic: white shapes on black, no glow
            return Look(vec3f(0.0), vec3f(1.0), vec3f(1.0), vec3f(0.55), vec3f(1.0),
                        vec3f(0.85), 0.0, 1.0, 0.0);
        }
    }
}

// signed distance to an axis-aligned box centred at c with half extents h
fn box_d(p: vec2f, c: vec2f, h: vec2f) -> f32 {
    let d = abs(p - c) - h;
    return length(max(d, vec2f(0.0))) + min(max(d.x, d.y), 0.0);
}

// a soft halo outside a shape, 1 at its edge, gone a few r away
fn halo(d: f32, r: f32) -> f32 {
    return exp(-max(d, 0.0) / max(r, 1e-5));
}

// the ball at time t: two bounces, x between the paddles, y between the walls
fn ball_pos(t: f32, vx: f32, vy: f32, ph: vec2f) -> vec2f {
    return vec2f(game_bounce(t, BX_LO, BX_HI, vx, ph.x),
                 game_bounce(t, BY_LO, BY_HI, vy, ph.y));
}

// a paddle's centre: it eases from the ball's height at the previous hit on its
// side to the ball's height at the next one, over the whole interval between
// them, so it glides and never teleports; clamped to stay inside the walls
fn paddle_y(t: f32, vx: f32, vy: f32, ph: vec2f, right: bool) -> f32 {
    let period = 2.0 * (BX_HI - BX_LO) / max(vx, 1e-5);
    let t_next = game_next_bounce(t, BX_LO, BX_HI, vx, ph.x, right);
    let t_prev = t_next - period;
    let lim = WALL_Y - WALL_HALF - PADDLE_H.y;
    let y_prev = clamp(game_bounce(t_prev, BY_LO, BY_HI, vy, ph.y), -lim, lim);
    let y_next = clamp(game_bounce(t_next, BY_LO, BY_HI, vy, ph.y), -lim, lim);
    return game_ease_to(y_prev, y_next, t, t_prev, period);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;
    let speed = 0.5 + k.x;                       // 1.0 as designed
    let vx = 0.55 * speed;
    let vy = 0.31 * speed;                       // incommensurate with vx: the path does not repeat soon
    // the serve: where in their cycles the two waves start, different every launch
    let h = hash_seeded2(vec2i(3, 7), 0x506f6e67u, ctx);
    let ph = vec2f(h.x * 2.0 * (BX_HI - BX_LO), h.y * 2.0 * (BY_HI - BY_LO));
    let trail_amt = k.y * 2.0 * l.trail_gain;    // 1.0 as designed (x theme gain)

    // background; the crt tube is a little brighter at its centre
    var col = l.bg * (1.0 + l.crt * 0.6 * exp(-dot(p, p) * 4.0));

    // the court: a dashed net down the middle (dashes 0.03 long every 0.06) and
    // two walls that run on forever left and right
    let ny = fmod_pos(p.y, 0.06) - 0.03;
    let d_net = box_d(vec2f(p.x, ny), vec2f(0.0), vec2f(0.006, 0.015));
    col = over(col, l.net, aa_fill(d_net, ctx));
    let d_wall = abs(abs(p.y) - WALL_Y) - WALL_HALF;
    col = over(col, l.wall, aa_fill(d_wall, ctx));

    // the score: each side counts the times the ball reached the far side, so
    // it is the half cycles of the x bounce (it rises lo -> hi in the first half)
    let bp = game_bounce_phase(t, BX_LO, BX_HI, vx, ph.x);
    let total = max(bp.x + bp.y, 0.0);
    let score_l = u32(floor(total + 0.5)) % 100u;      // ball has reached the right side
    let score_r = u32(floor(total)) % 100u;            // ball has reached the left side
    let gs = 0.01;                                     // glyph pixel: digits 0.05 tall
    let gl = glyph_locate(p, vec2f(-0.14, 0.39), gs);
    let gr = glyph_locate(p, vec2f(0.07, 0.39), gs);
    var txt = glyph_number(score_l, 2u, false, i32(gl.x), gl.y, gl.z);
    txt = max(txt, glyph_number(score_r, 2u, false, i32(gr.x), gr.y, gr.z));
    col = over(col, l.text, txt);

    // the ball's trail: fainter squares where it was 30 ms steps ago
    for (var i = 1; i <= 6; i++) {
        let bt = ball_pos(t - f32(i) * 0.03, vx, vy, ph);
        let a = (1.0 - f32(i) / 7.0) * trail_amt * 0.5;
        col = over(col, l.ball, aa_fill(box_d(p, bt, vec2f(BALL_HALF)), ctx) * a);
    }

    // the ball, with a halo in the glowing themes
    let b = ball_pos(t, vx, vy, ph);
    let d_ball = box_d(p, b, vec2f(BALL_HALF));
    col += l.ball * (l.glow * 0.3 * halo(d_ball, 0.035));
    col = over(col, l.ball, aa_fill(d_ball, ctx));

    // the paddles, gliding to the ball's next arrival
    let yl = paddle_y(t, vx, vy, ph, false);
    let yr = paddle_y(t, vx, vy, ph, true);
    let d_pad = min(box_d(p, vec2f(-PADDLE_X, yl), PADDLE_H),
                    box_d(p, vec2f(PADDLE_X, yr), PADDLE_H));
    col += l.paddle * (l.glow * 0.12 * halo(d_pad, 0.03));
    col = over(col, l.paddle, aa_fill(d_pad, ctx));

    // crt: scanlines (fading out where they would alias at terminal size) and a
    // faint vignette darkening toward |p| = 0.6
    if (l.crt > 0.0) {
        let depth = 0.35 * saturate((1.0 / 140.0) / (2.5 * ctx.px));
        col *= game_scanlines(p, 140.0, depth);
        col *= mix(1.0, 0.5, sstep(0.3, 0.7, length(p)));
    }
    return max(col, vec3f(0.0));
}
