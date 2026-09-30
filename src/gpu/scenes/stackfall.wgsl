//! name: stackfall
//! title: Falling Blocks
//! category: games
//! tags: blocks, tetromino, puzzle, retro, arcade, game
//! desc: blocks fall into a well and stack up, rows flash and clear, and it starts again
//! themes: classic, pastel, mono
//! params: speed
//! uses: game, glyph, hash
//! cost: light
//! fallback: sand
//! credits: original

// A falling-blocks puzzle that plays itself. A round is 14 hashed tetrominoes
// dropped one after another into a 10x20 well. Nothing is stored: every pixel
// re-simulates the round's stack from its hash (a bounded loop over at most
// 14 pieces), so the falling piece, the stack and the line-clear wave are all
// pure functions of ctx.t.

const COLS: i32 = 10;
const ROWS: i32 = 20;
const CELL: f32 = 0.04;
const NPIECES: i32 = 14;
const PLAY: f32 = 0.92;      // share of the round spent dropping pieces; the rest clears

struct Look {
    bg: vec3f,        // far background
    well: vec3f,      // inside the well and the panels
    frame: vec3f,     // the well's frame and the panel borders
    grid: vec3f,      // faint grid inside the well
    ink: vec3f,       // text
    flash: vec3f,     // landing and line-clear flash
    rubble: vec3f,    // the round's starting rubble
    mono: f32,        // 1 = one colour, brightness by piece age
    scan: f32,        // scanline depth
}

fn look(theme: u32) -> Look {
    switch (theme) {
        case 1u: {  // pastel: soft colours on dark slate, a warm wooden frame
            return Look(col_hex(0x1a1e26u), col_hex(0x2b3440u), col_hex(0x8a6a4au), col_hex(0x36414eu),
                        col_hex(0xf3e6d0u), col_hex(0xfff3e0u), col_hex(0x6f7a86u), 0.0, 0.0);
        }
        case 2u: {  // mono: one amber phosphor, newest piece brightest, faint scanlines
            return Look(col_hex(0x120d06u), col_hex(0x100c05u), col_hex(0x3a2a10u), col_hex(0x1f170au),
                        col_hex(0xffb000u), col_hex(0xffe0a0u), col_hex(0xffb000u), 1.0, 0.3);
        }
        default: {  // classic: the seven arcade colours on black
            return Look(col_hex(0x0b0d14u), col_hex(0x000000u), col_hex(0x2a2f3au), col_hex(0x14171eu),
                        col_hex(0xe8e8f0u), col_hex(0xffffffu), col_hex(0x5a5f6au), 0.0, 0.0);
        }
    }
}

// the colour of tetromino `s` (I O T S Z J L) in this theme
fn piece_col(theme: u32, s: i32) -> vec3f {
    if (theme == 2u) { return col_hex(0xffb000u); }
    let pastel = theme == 1u;
    switch (s) {
        case 0: { return select(col_hex(0x00e0f0u), col_hex(0x9fd8e8u), pastel); }   // I
        case 1: { return select(col_hex(0xf0e000u), col_hex(0xf7e29bu), pastel); }   // O
        case 2: { return select(col_hex(0xb040f0u), col_hex(0xd3b2f0u), pastel); }   // T
        case 3: { return select(col_hex(0x20e040u), col_hex(0xb5e8b0u), pastel); }   // S
        case 4: { return select(col_hex(0xf03030u), col_hex(0xf2a7a7u), pastel); }   // Z
        case 5: { return select(col_hex(0x3050f0u), col_hex(0xa7b8f2u), pastel); }   // J
        default: { return select(col_hex(0xf09020u), col_hex(0xf5c9a0u), pastel); }  // L
    }
}

// the 4 cells of a tetromino packed 4 bits each: dx | (dy << 2), cell k in bits 4k..4k+3
fn shape_bits(s: i32) -> u32 {
    switch (s) {
        case 0: { return 0x3210u; }    // I  ####
        case 1: { return 0x5410u; }    // O  ## / ##
        case 2: { return 0x5210u; }    // T  ### with a bump in the middle
        case 3: { return 0x6510u; }    // S
        case 4: { return 0x5421u; }    // Z
        case 5: { return 0x4210u; }    // J
        default: { return 0x6210u; }   // L
    }
}
fn shape_width(s: i32) -> i32 { return select(select(3, 2, s == 1), 4, s == 0); }
fn cell_of(bits: u32, k: i32) -> vec2i {
    let n = (bits >> u32(k * 4)) & 15u;
    return vec2i(i32(n & 3u), i32(n >> 2u));
}

struct Piece { s: i32, c: i32, bits: u32 }

// piece j of round r: shape and target column from the hash (per launch)
fn piece_at(r: i32, j: i32, ctx: Ctx) -> Piece {
    let h = hash_seeded2(vec2i(j, r), 1234u, ctx);
    let s = clamp(i32(h.x * 7.0), 0, 6);
    let w = shape_width(s);
    let c = clamp(i32(h.y * f32(COLS - w + 1)), 0, COLS - w);
    return Piece(s, c, shape_bits(s));
}

// rubble height (0..3) column `col` starts the round with
fn base_h(col: i32, r: i32, ctx: Ctx) -> i32 {
    return clamp(i32(hash_seeded2(vec2i(col, r), 77u, ctx).x * 4.0), 0, 3);
}

// bevel: a lighter top-left edge and a darker bottom-right one, fading away
// once a cell is only a few pixels wide
fn block_shade(uv: vec2f, aa_uv: f32) -> f32 {
    let e = 0.18;
    let br = max(step(uv.y, e), step(1.0 - e, uv.x));
    let tl = max(step(1.0 - e, uv.y), step(uv.x, e)) * (1.0 - br);
    let lod = saturate(1.0 - aa_uv * 3.0);
    return 1.0 + (0.35 * tl - 0.4 * br) * lod;
}

// a five-letter label (pass 32u for blanks): coverage of p
fn label5(p: vec2f, origin: vec2f, size: f32, c0: u32, c1: u32, c2: u32, c3: u32, c4: u32) -> f32 {
    let loc = glyph_locate(p, origin, size);
    let cell = i32(loc.x);
    if (cell < 0 || cell > 4) { return 0.0; }
    var ch = c4;
    switch (cell) {
        case 0: { ch = c0; }
        case 1: { ch = c1; }
        case 2: { ch = c2; }
        case 3: { ch = c3; }
        default: { ch = c4; }
    }
    return glyph_sample(glyph_font(ch), loc.y, loc.z);
}

fn number_at(p: vec2f, origin: vec2f, size: f32, value: u32, digits: u32) -> f32 {
    let loc = glyph_locate(p, origin, size);
    return glyph_number(value, digits, false, i32(loc.x), loc.y, loc.z);
}

fn scene(p: vec2f, ctx: Ctx) -> vec3f {
    let l = look(ctx.theme);
    let t = ctx.t;
    let k = ctx.params;

    // ---- the clock of the game
    let round_len = 30.0 / (0.5 + k.x);                    // 30 s as designed
    let slot = round_len * PLAY / f32(NPIECES);            // one piece per slot
    let r = i32(floor(t / round_len));
    let tl = t - f32(r) * round_len;
    let j = min(i32(floor(tl / slot)), NPIECES);           // 14 = everything landed, clearing
    let u = fract(tl / slot);                              // progress inside the slot
    let w = saturate((tl / round_len - PLAY) / (1.0 - PLAY));  // line-clear progress
    let clearing = w > 0.0;

    // ---- background: dark, with a very faint pattern of big tiles so wide
    // and tall framings are never empty; a slow pulse per tile
    let tile = floor(p / 0.16);
    let ht = hash_cell2(vec2i(tile), 9u);
    let pulse_t = 0.5 + 0.5 * sin(t * 0.25 + ht.y * TAU);
    var col = l.bg * (0.6 + 1.4 * ht.x * pulse_t);
    // a hint of a diagonal weave over the tiles
    col *= 1.0 + 0.08 * sin((p.x + p.y) * 40.0);

    // ---- the well: frame, then the interior
    let well_half = vec2f(f32(COLS), f32(ROWS)) * CELL * 0.5;   // (0.2, 0.4)
    let origin = -well_half;
    // the frame glows a little when the wave clears the well
    let clear_pulse = sin(w * PI) * select(0.0, 1.0, clearing);
    let outside = length(max(abs(p) - well_half, vec2f(0.0)));
    col += l.flash * 0.06 * clear_pulse * exp(-outside * 10.0);
    let fr = game_rect(p, vec2f(0.0), well_half + vec2f(0.025), ctx.px);
    let frame_col = l.frame * (0.9 + 0.3 * sstep(-0.45, 0.45, p.y));
    col = mix(col, frame_col, fr);

    // side panels: level on the left, next piece and score on the right
    let lp_c = vec2f(-0.355, 0.26);
    let rp_c = vec2f(0.355, 0.18);
    let lp = game_rect(p, lp_c, vec2f(0.105, 0.10), ctx.px);
    let rp = game_rect(p, rp_c, vec2f(0.105, 0.18), ctx.px);
    col = mix(col, frame_col, max(lp, rp));
    let lpi = game_rect(p, lp_c, vec2f(0.095, 0.09), ctx.px);
    let rpi = game_rect(p, rp_c, vec2f(0.095, 0.17), ctx.px);
    col = mix(col, l.well, max(lpi, rpi));

    // ---- simulate the round: rubble, then pieces 0..j-1 landed, piece j falling
    let g = game_grid(p, origin, CELL);
    let cx = i32(g.x);
    let cy = i32(g.y);
    let uv = g.zw;
    let in_well = cx >= 0 && cx < COLS && cy >= 0 && cy < ROWS;

    var hgt: array<i32, 10>;
    for (var c = 0; c < COLS; c++) { hgt[c] = base_h(c, r, ctx); }
    var hit = 0.0;                        // this cell holds a landed block
    var hit_col = l.rubble;
    var hit_age = 0.0;                    // 0 oldest .. 1 newest (mono brightness)
    if (in_well && cy < hgt[clamp(cx, 0, COLS - 1)]) { hit = 1.0; }

    var land = 0;
    var cur = Piece(0, 0, 0u);
    for (var i = 0; i < NPIECES; i++) {
        if (i > j) { break; }
        let pc = piece_at(r, i, ctx);
        // it stops on the first row where none of its cells is inside the stack
        var lrow = 0;
        for (var q = 0; q < 4; q++) {
            let o = cell_of(pc.bits, q);
            lrow = max(lrow, hgt[clamp(pc.c + o.x, 0, COLS - 1)] - o.y);
        }
        if (i == j) { land = lrow; cur = pc; break; }      // the piece in the air
        for (var q = 0; q < 4; q++) {
            let o = cell_of(pc.bits, q);
            let ccol = clamp(pc.c + o.x, 0, COLS - 1);
            hgt[ccol] = max(hgt[ccol], lrow + o.y + 1);
            if (cx == ccol && cy == lrow + o.y) {
                hit = 1.0;
                hit_col = piece_col(ctx.theme, pc.s);
                hit_age = f32(i + 1) / f32(NPIECES);
            }
        }
    }

    // the falling piece: one cell at a time, like the real game, then a tiny flash
    var fall = 0.0;
    var flash = 0.0;
    if (j < NPIECES) {
        let uf = saturate(u / 0.85);
        let row_now = land + i32(ceil((1.0 - uf) * f32(max(ROWS - land, 0))));
        for (var q = 0; q < 4; q++) {
            let o = cell_of(cur.bits, q);
            if (cx == cur.c + o.x && cy == row_now + o.y) { fall = 1.0; }
        }
        flash = select(0.0, 1.0 - sstep(0.85, 1.0, u), uf >= 1.0);
    }

    // ---- draw the well interior
    let aa_uv = ctx.px / CELL;
    var wcol = l.well;
    let gl = max(step(uv.x, aa_uv), step(uv.y, aa_uv)) * saturate(1.0 - aa_uv * 4.0);
    wcol = mix(wcol, l.grid, gl);
    let body = game_rect(uv, vec2f(0.5), vec2f(0.47), aa_uv);
    let shade = block_shade(uv, aa_uv);

    var stack_a = hit;
    var stack_c = hit_col * mix(1.0, mix(0.3, 1.0, hit_age), l.mono);
    if (clearing) {
        // a white wave climbs the well; the stack fades away behind it
        let front = mix(-3.0, f32(ROWS) + 3.0, sstep(0.0, 0.6, w));
        let glow = exp(-abs(f32(cy) + 0.5 - front) * 0.5);
        stack_c = mix(stack_c, l.flash * 1.6, glow * 0.9);
        stack_a *= 1.0 - sstep(1.5, 4.0, front - f32(cy));
    }
    wcol = mix(wcol, stack_c * shade, stack_a * body);
    // the next round's rubble fades in as the well empties
    let nxt_rubble = select(0.0, 1.0, in_well && cy < base_h(cx, r + 1, ctx)) * sstep(0.7, 1.0, w);
    let rubble_c = l.rubble * mix(1.0, 0.3, l.mono) * shade;
    wcol = mix(wcol, rubble_c, nxt_rubble * body * (1.0 - stack_a));
    // the piece in the air, brightest of all in mono
    let fcol = mix(piece_col(ctx.theme, cur.s) * shade, l.flash * 1.5, 0.6 * flash);
    wcol = mix(wcol, fcol, fall * body);

    let inside = game_rect(p, vec2f(0.0), well_half, ctx.px);
    col = mix(col, wcol, inside);

    // ---- the panels' contents
    let score = u32(max(r, 0)) * 1000u + u32(min(j, NPIECES)) * 100u;
    let level = u32(max(r, 0) + 1);
    var ink = 0.0;
    ink = max(ink, label5(p, vec2f(-0.445, 0.335), 0.009, 76u, 69u, 86u, 69u, 76u));     // LEVEL
    ink = max(ink, number_at(p, vec2f(-0.41, 0.27), 0.015, level, 2u));
    ink = max(ink, label5(p, vec2f(0.265, 0.335), 0.009, 78u, 69u, 88u, 84u, 32u));      // NEXT
    ink = max(ink, label5(p, vec2f(0.265, 0.14), 0.009, 83u, 67u, 79u, 82u, 69u));       // SCORE
    ink = max(ink, number_at(p, vec2f(0.262, 0.075), 0.008, score, 6u));
    col = mix(col, l.ink, ink);

    // NEXT: the piece after this one (or the next round's first)
    let wrap = j + 1 >= NPIECES;
    let np = piece_at(select(r, r + 1, wrap), select(j + 1, 0, wrap), ctx);
    let pcell = 0.03;
    let porigin = vec2f(0.355 - f32(shape_width(np.s)) * pcell * 0.5, 0.20);
    let pg = game_grid(p, porigin, pcell);
    var pv = 0.0;
    for (var q = 0; q < 4; q++) {
        let o = cell_of(np.bits, q);
        if (i32(pg.x) == o.x && i32(pg.y) == o.y) { pv = 1.0; }
    }
    let paa = ctx.px / pcell;
    let pbody = game_rect(pg.zw, vec2f(0.5), vec2f(0.46), paa);
    col = mix(col, piece_col(ctx.theme, np.s) * block_shade(pg.zw, paa), pv * pbody);

    // mono: faint scanlines, dropped when the output is too coarse to hold them
    col *= game_scanlines(p, 60.0, l.scan * saturate(1.5 - ctx.px * 180.0));
    return max(col, vec3f(0.0));
}
