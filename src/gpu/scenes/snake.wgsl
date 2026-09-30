//! name: snake
//! title: Snake
//! category: games
//! tags: snake, retro, handheld, grid, game, nokia
//! desc: a snake that mows the board row by row, eating as it grows, on a glowing grid
//! themes: classic, neon, handheld
//! params: speed, grid
//! uses: game, glyph, hash, light
//! cost: light
//! fallback: pipes
//! credits: original

// A self-playing snake that can never collide: its path is a boustrophedon
// sweep of the board (right along even rows, left along odd rows, one step
// down between rows). Every step of the sweep is a cell, so the whole game is
// a function of the tick index: head = cell of the current step, body = the
// cells of the last `len` steps, pellet = the cell of the next multiple of 12
// steps (eaten exactly when the snake grows). When the sweep wraps to the top
// the snake re-emerges from the corner: a new game.

struct Look {
    bg: vec3f,      // beyond the board
    board: vec3f,   // the board fill
    grid: vec3f,    // grid lines
    frame: vec3f,   // the board frame
    snake: vec3f,   // body
    head: vec3f,    // head (brightest)
    pellet: vec3f,
    text: vec3f,    // score
    glow: f32,      // soft halo around the snake (neon)
    gridw: f32,     // grid line half width in cells
    grida: f32,     // grid line strength
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {  // neon: deep indigo, cyan snake with a halo, magenta pellet
            return Look(col_hex(0x06051au), col_hex(0x0e0b30u), col_hex(0x3a2a86u), col_hex(0x7a58f0u),
                        col_hex(0x18e0ffu), col_hex(0xc0ffffu), col_hex(0xff30c8u), col_hex(0xa0f4ffu),
                        1.0, 0.03, 0.55);
        }
        case 2u: {  // handheld: the four-tone green LCD
            return Look(col_hex(0x0a2a0au), col_hex(0x0f380fu), col_hex(0x306230u), col_hex(0x306230u),
                        col_hex(0x8bac0fu), col_hex(0x9bbc0fu), col_hex(0x9bbc0fu), col_hex(0x9bbc0fu),
                        0.0, 0.07, 1.0);
        }
        default: {  // classic: black board, bright green snake, red pellet, grey lines
            return Look(col_hex(0x0c0c10u), col_hex(0x000000u), col_hex(0x303036u), col_hex(0x9a9aa2u),
                        col_hex(0x30e030u), col_hex(0xa0ffa0u), col_hex(0xff3020u), col_hex(0xd8ffd8u),
                        0.0, 0.03, 0.6);
        }
    }
}

// the cell (column, row from the top) the head occupies at sweep step `s`
fn cell_of_step(s: i32, w: i32) -> vec2i {
    let r = s / w;
    let j = s - r * w;
    let c = select(w - 1 - j, j, (r & 1) == 0);
    return vec2i(c, r);
}

// the sweep step at which the head visits cell (column, row from the top)
fn step_of_cell(cell: vec2i, w: i32) -> i32 {
    let j = select(w - 1 - cell.x, cell.x, (cell.y & 1) == 0);
    return cell.y * w + j;
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;

    // ---- board geometry: the grid slider scales the cells 0.6x .. 1.6x
    let zoom = select(mix(0.6, 1.0, k.y * 2.0), mix(1.0, 1.6, (k.y - 0.5) * 2.0), k.y > 0.5);
    let nh = max(i32(round(18.0 / zoom)), 6);
    let nw = max(i32(round(32.0 / zoom)), 8);
    let c = 0.8 / f32(nh);
    let bh = vec2f(f32(nw), f32(nh)) * c * 0.5;
    let origin = -bh;
    let aac = ctx.px / c;   // one pixel in cell units

    // ---- the game clock
    let hz = 6.0 * (0.5 + k.x);
    let tk = game_tick(t, hz);
    let per = nw * nh;
    let sf = fmod_pos(tk.x, f32(per));
    let s = i32(sf);
    let len = min(4 + s / 12, 40);
    let head = cell_of_step(s, nw);
    let sp0 = (s / 12 + 1) * 12;
    let sp = sp0 - (sp0 / per) * per;
    let pel = cell_of_step(sp, nw);
    // head direction: down on the first cell of a row, else along the row
    let j_head = s - head.y * nw;
    var dir = vec2f(select(-1.0, 1.0, (head.y & 1) == 0), 0.0);
    if (j_head == 0 && head.y > 0) { dir = vec2f(0.0, -1.0); }
    // between games: one gentle swell of the frame over the last 8 steps
    let uf = (sf + tk.y - f32(per - 8)) / 8.0;
    let flash = select(0.0, sin(saturate(uf) * PI), uf > 0.0);

    // ---- which cell is this pixel in
    let g = game_grid(p, origin, c);
    let cell = vec2i(i32(g.x), i32(g.y));
    let uv = g.zw;
    let row = nh - 1 - cell.y;
    let inside = cell.x >= 0 && cell.x < nw && row >= 0 && row < nh;

    // ---- background and the grid, which continues beyond the board
    var col = select(l.bg, l.board, inside);
    col *= 1.0 - 0.55 * saturate((length(p) - 0.75) * 0.7);
    let e = min(min(uv.x, 1.0 - uv.x), min(uv.y, 1.0 - uv.y)) * c;
    let lw = max(l.gridw * c, ctx.px * 0.6);
    let gl = aa_fill(e - lw, ctx);
    col = mix(col, l.grid, gl * l.grida * select(0.4, 1.0, inside));

    // ---- the snake and the pellet
    if (inside) {
        let sc = step_of_cell(vec2i(cell.x, row), nw);
        let age = s - sc;   // 0 = head, 1..len = body, else not part of the snake
        let on_snake = age >= 0 && age <= len;
        if (on_snake) {
            let fade = 1.0 - 0.45 * f32(age) / f32(max(len, 1));
            let seg = game_rect(uv, vec2f(0.5), vec2f(0.45), aac);
            var scol = l.snake * fade;
            if (age == 0) {
                scol = l.head;
                // two eye pixels toward the front
                let front = vec2f(0.5) + dir * 0.24;
                let side = vec2f(-dir.y, dir.x) * 0.2;
                let e1 = game_rect(uv, front + side, vec2f(0.09), aac);
                let e2 = game_rect(uv, front - side, vec2f(0.09), aac);
                scol = mix(scol, l.board, max(e1, e2) * 0.9);
            } else {
                // a darker inner square so segments read at low resolution
                let inner = game_rect(uv, vec2f(0.5), vec2f(0.27), aac);
                scol *= 1.0 - 0.3 * inner;
            }
            col = mix(col, scol, seg);
        } else if (cell.x == pel.x && row == pel.y) {
            let ps = 0.26 + 0.07 * sin(t * 5.0);
            let pr = game_rect(uv, vec2f(0.5), vec2f(ps), aac);
            col = mix(col, l.pellet, pr);
        }
    }

    // ---- neon halo along the body and around the pellet
    if (l.glow > 0.0) {
        var acc = 0.0;
        for (var i = 0; i < 40; i++) {
            if (i > len || i > s) { break; }
            let sc = cell_of_step(s - i, nw);
            let ctr = origin + (vec2f(f32(sc.x), f32(nh - 1 - sc.y)) + 0.5) * c;
            let w = 1.0 - 0.6 * f32(i) / f32(max(len, 1));
            acc = max(acc, light_glow(length(p - ctr), c * 0.9) * w);
        }
        col += l.snake * acc * 0.45 * l.glow;
        let pc = origin + (vec2f(f32(pel.x), f32(nh - 1 - pel.y)) + 0.5) * c;
        col += l.pellet * light_glow(length(p - pc), c * 0.8) * 0.35 * l.glow;
    }

    // ---- the frame, flashing once between games
    let db = abs(p) - bh;
    let dframe = max(db.x, db.y);
    let fw = max(c * 0.07, ctx.px * 0.7);
    let fr = aa_fill(abs(dframe - fw) - fw, ctx);
    col = mix(col, l.frame * (1.0 + 2.0 * flash), fr);
    col += l.frame * flash * 0.25 * light_glow(max(dframe, 0.0), c * 1.5) * f32(!inside);

    // ---- score, 3x5 glyphs in the margin above the board, flush right: length * 10
    let gs = c * 0.38;
    let sorg = vec2f(bh.x - 11.0 * gs, bh.y + 0.25 * c + 5.0 * gs);
    let loc = glyph_locate(p, sorg, gs);
    let txt = glyph_number(u32(len) * 10u, 3u, false, i32(loc.x), loc.y, loc.z);
    col = mix(col, l.text, txt);

    return max(col, vec3f(0.0));
}
