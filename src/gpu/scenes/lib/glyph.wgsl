// ---------------------------------------------------------------- glyph
// Text and sprites at terminal resolution. A 3x5 bitmap font (digits, the
// capitals, a few symbols), hashed pseudo-glyphs that read as text from a
// distance, and bit-row sprites. Everything is integer and hash based, so
// every pane of a wall draws the same letters.
//
// A glyph is 15 bits in a u32: row r (0 = top), column c (0 = left) is bit
// r * 3 + (2 - c), so a row written as 0b101 has its lit pixels on the
// outside. Cells are 4 wide (3 + a gap) and 6 tall (5 + a gap): a string is
// a row of cells, `glyph_locate` says which cell and where inside it p is.

fn glyph_rows(r0: u32, r1: u32, r2: u32, r3: u32, r4: u32) -> u32 {
    return r0 | (r1 << 3u) | (r2 << 6u) | (r3 << 9u) | (r4 << 12u);
}

// the glyph for an ASCII code (digits, capitals, space and punctuation);
// lower-case letters draw as capitals, anything unknown as a small box
fn glyph_font(ch: u32) -> u32 {
    var c = ch;
    if (c >= 97u && c <= 122u) { c -= 32u; }
    switch (c) {
        case 48u: { return glyph_rows(7u, 5u, 5u, 5u, 7u); }   // 0
        case 49u: { return glyph_rows(2u, 6u, 2u, 2u, 7u); }   // 1
        case 50u: { return glyph_rows(7u, 1u, 7u, 4u, 7u); }   // 2
        case 51u: { return glyph_rows(7u, 1u, 7u, 1u, 7u); }   // 3
        case 52u: { return glyph_rows(5u, 5u, 7u, 1u, 1u); }   // 4
        case 53u: { return glyph_rows(7u, 4u, 7u, 1u, 7u); }   // 5
        case 54u: { return glyph_rows(7u, 4u, 7u, 5u, 7u); }   // 6
        case 55u: { return glyph_rows(7u, 1u, 1u, 1u, 1u); }   // 7
        case 56u: { return glyph_rows(7u, 5u, 7u, 5u, 7u); }   // 8
        case 57u: { return glyph_rows(7u, 5u, 7u, 1u, 7u); }   // 9
        case 65u: { return glyph_rows(2u, 5u, 7u, 5u, 5u); }   // A
        case 66u: { return glyph_rows(6u, 5u, 6u, 5u, 6u); }   // B
        case 67u: { return glyph_rows(7u, 4u, 4u, 4u, 7u); }   // C
        case 68u: { return glyph_rows(6u, 5u, 5u, 5u, 6u); }   // D
        case 69u: { return glyph_rows(7u, 4u, 7u, 4u, 7u); }   // E
        case 70u: { return glyph_rows(7u, 4u, 7u, 4u, 4u); }   // F
        case 71u: { return glyph_rows(7u, 4u, 5u, 5u, 7u); }   // G
        case 72u: { return glyph_rows(5u, 5u, 7u, 5u, 5u); }   // H
        case 73u: { return glyph_rows(7u, 2u, 2u, 2u, 7u); }   // I
        case 74u: { return glyph_rows(1u, 1u, 1u, 5u, 7u); }   // J
        case 75u: { return glyph_rows(5u, 5u, 6u, 5u, 5u); }   // K
        case 76u: { return glyph_rows(4u, 4u, 4u, 4u, 7u); }   // L
        case 77u: { return glyph_rows(5u, 7u, 7u, 5u, 5u); }   // M
        case 78u: { return glyph_rows(6u, 5u, 5u, 5u, 5u); }   // N
        case 79u: { return glyph_rows(7u, 5u, 5u, 5u, 7u); }   // O
        case 80u: { return glyph_rows(7u, 5u, 7u, 4u, 4u); }   // P
        case 81u: { return glyph_rows(7u, 5u, 5u, 7u, 1u); }   // Q
        case 82u: { return glyph_rows(6u, 5u, 6u, 5u, 5u); }   // R
        case 83u: { return glyph_rows(7u, 4u, 7u, 1u, 7u); }   // S
        case 84u: { return glyph_rows(7u, 2u, 2u, 2u, 2u); }   // T
        case 85u: { return glyph_rows(5u, 5u, 5u, 5u, 7u); }   // U
        case 86u: { return glyph_rows(5u, 5u, 5u, 5u, 2u); }   // V
        case 87u: { return glyph_rows(5u, 5u, 7u, 7u, 5u); }   // W
        case 88u: { return glyph_rows(5u, 5u, 2u, 5u, 5u); }   // X
        case 89u: { return glyph_rows(5u, 5u, 2u, 2u, 2u); }   // Y
        case 90u: { return glyph_rows(7u, 1u, 2u, 4u, 7u); }   // Z
        case 32u: { return 0u; }                               // space
        case 33u: { return glyph_rows(2u, 2u, 2u, 0u, 2u); }   // !
        case 34u: { return glyph_rows(5u, 5u, 0u, 0u, 0u); }   // "
        case 35u: { return glyph_rows(5u, 7u, 5u, 7u, 5u); }   // #
        case 36u: { return glyph_rows(7u, 6u, 7u, 3u, 7u); }   // $
        case 37u: { return glyph_rows(5u, 1u, 2u, 4u, 5u); }   // %
        case 39u: { return glyph_rows(2u, 2u, 0u, 0u, 0u); }   // '
        case 40u: { return glyph_rows(2u, 4u, 4u, 4u, 2u); }   // (
        case 41u: { return glyph_rows(2u, 1u, 1u, 1u, 2u); }   // )
        case 42u: { return glyph_rows(5u, 2u, 7u, 2u, 5u); }   // *
        case 43u: { return glyph_rows(0u, 2u, 7u, 2u, 0u); }   // +
        case 44u: { return glyph_rows(0u, 0u, 0u, 2u, 4u); }   // ,
        case 45u: { return glyph_rows(0u, 0u, 7u, 0u, 0u); }   // -
        case 46u: { return glyph_rows(0u, 0u, 0u, 0u, 2u); }   // .
        case 47u: { return glyph_rows(1u, 1u, 2u, 4u, 4u); }   // /
        case 58u: { return glyph_rows(0u, 2u, 0u, 2u, 0u); }   // :
        case 59u: { return glyph_rows(0u, 2u, 0u, 2u, 4u); }   // ;
        case 60u: { return glyph_rows(1u, 2u, 4u, 2u, 1u); }   // <
        case 61u: { return glyph_rows(0u, 7u, 0u, 7u, 0u); }   // =
        case 62u: { return glyph_rows(4u, 2u, 1u, 2u, 4u); }   // >
        case 63u: { return glyph_rows(7u, 1u, 3u, 0u, 2u); }   // ?
        case 64u: { return glyph_rows(7u, 5u, 7u, 4u, 7u); }   // @
        case 91u: { return glyph_rows(6u, 4u, 4u, 4u, 6u); }   // [
        case 92u: { return glyph_rows(4u, 4u, 2u, 1u, 1u); }   // backslash
        case 93u: { return glyph_rows(3u, 1u, 1u, 1u, 3u); }   // ]
        case 94u: { return glyph_rows(2u, 5u, 0u, 0u, 0u); }   // ^
        case 95u: { return glyph_rows(0u, 0u, 0u, 0u, 7u); }   // _
        case 124u: { return glyph_rows(2u, 2u, 2u, 2u, 2u); }  // |
        case 126u: { return glyph_rows(0u, 3u, 7u, 6u, 0u); }  // ~
        default: { return glyph_rows(7u, 5u, 5u, 5u, 7u); }
    }
}

// pixel (x, y) of a glyph, x 0..2 left to right, y 0..4 top to bottom
fn glyph_bit(g: u32, x: i32, y: i32) -> f32 {
    if (x < 0 || x > 2 || y < 0 || y > 4) { return 0.0; }
    return f32((g >> u32(y * 3 + (2 - x))) & 1u);
}

// the glyph sampled inside its 4x6 cell: (lx, ly) in cell units, y down
fn glyph_sample(g: u32, lx: f32, ly: f32) -> f32 {
    return glyph_bit(g, i32(floor(lx)), i32(floor(ly)));
}

// Where p falls in a line of text whose top-left cell corner is `origin`
// (p units, y up) with cells `size` p units per glyph pixel: returns (cell
// index, x inside the cell 0..4, y inside the cell 0..6), the index -1 above
// or below the line and left of it.
fn glyph_locate(p: vec2f, origin: vec2f, size: f32) -> vec3f {
    let q = vec2f(p.x - origin.x, origin.y - p.y) / max(size, 1e-6);
    let col = floor(q.x / 4.0);
    let lx = q.x - col * 4.0;
    if (q.y < 0.0 || q.y >= 6.0 || q.x < 0.0) { return vec3f(-1.0, lx, q.y); }
    return vec3f(col, lx, q.y);
}

// A number right-aligned in `digits` cells (leading zeros when `pad`):
// coverage at (cell index, lx, ly) from `glyph_locate`.
fn glyph_number(value: u32, digits: u32, pad: bool, cell: i32, lx: f32, ly: f32) -> f32 {
    if (cell < 0 || cell >= i32(digits)) { return 0.0; }
    // the digit `place` from the right in this cell
    let place = i32(digits) - 1 - cell;
    var v = value;
    for (var i = 0; i < 10; i++) {
        if (i >= place) { break; }
        v /= 10u;
    }
    if (v == 0u && place > 0 && !pad) {
        // no digit here unless the number reaches this far
        var probe = value;
        for (var i = 0; i < 10; i++) {
            if (i >= place) { break; }
            probe /= 10u;
        }
        if (probe == 0u) { return 0.0; }
    }
    return glyph_sample(glyph_font(48u + v % 10u), lx, ly);
}

// A hashed glyph that reads as a letter of an unknown alphabet: about half
// the pixels lit, never an empty row, symmetric-ish so it looks designed.
fn glyph_pseudo(id: u32) -> u32 {
    let h = hash_u(id * 0x9e3779b9u + 0x7f4a7c15u);
    var g = h & 0x7fffu;
    // every row keeps at least its middle or outer pixels
    for (var r = 0u; r < 5u; r++) {
        let row = (g >> (r * 3u)) & 7u;
        if (row == 0u) { g |= select(2u, 5u, ((h >> (16u + r)) & 1u) == 1u) << (r * 3u); }
    }
    // a bar across the top or the bottom, like a serif alphabet
    if (((h >> 21u) & 3u) == 0u) { g |= 7u; }
    if (((h >> 23u) & 3u) == 0u) { g |= 7u << 12u; }
    return g;
}

// A bit-row sprite: pixel x (0 = left) of a row whose bits are `row`, the
// sprite `width` pixels wide with its leftmost pixel in the highest bit.
fn glyph_rowbit(row: u32, x: i32, width: i32) -> f32 {
    if (x < 0 || x >= width) { return 0.0; }
    return f32((row >> u32(width - 1 - x)) & 1u);
}
