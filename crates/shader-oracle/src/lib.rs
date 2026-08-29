//! CPU reimplementations of the workspace's shader math, the
//! independent-formula side of the render done-tests: the GPU renders a
//! pixel, this crate computes what that pixel should be from the same
//! closed-form expressions, and a test compares the two. Every function is
//! a line-for-line translation of its `.slang` source (the metal-surface
//! family in `crates/chassis/shaders/metal/metal_common.slang` and its
//! consumers, the glass chain's `terminal_frame` and bloom passes), not an
//! independent re-derivation.
//!
//! Dev-dependency only: nothing here ships in a binary. The uniform payload
//! structs the functions take live with the production code in
//! [`chassis::params`] and are re-exported here for the tests' convenience.

pub use chassis::params::{ChassisMetalParams, FrameMetalParams, MetalParams, PlateMetalParams};

pub fn hash12(p: [f32; 2]) -> f32 {
    let mut p3 = [
        frac(p[0] * 0.1031),
        frac(p[1] * 0.1031),
        frac(p[0] * 0.1031),
    ];
    let d = p3[0] * (p3[1] + 33.33) + p3[1] * (p3[2] + 33.33) + p3[2] * (p3[0] + 33.33);
    p3[0] += d;
    p3[1] += d;
    p3[2] += d;
    frac((p3[0] + p3[1]) * p3[2])
}

fn frac(x: f32) -> f32 {
    x - x.floor()
}

fn mix1(a: f32, b: f32, t: f32) -> f32 {
    a * (1.0 - t) + b * t
}

fn smoothstep(min: f32, max: f32, x: f32) -> f32 {
    let t = ((x - min) / (max - min)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub fn vnoise(p: [f32; 2]) -> f32 {
    let i = [p[0].floor(), p[1].floor()];
    let f = [p[0] - i[0], p[1] - i[1]];
    let u = [
        f[0] * f[0] * (3.0 - 2.0 * f[0]),
        f[1] * f[1] * (3.0 - 2.0 * f[1]),
    ];
    let a = hash12(i);
    let b = hash12([i[0] + 1.0, i[1]]);
    let c = hash12([i[0], i[1] + 1.0]);
    let d = hash12([i[0] + 1.0, i[1] + 1.0]);
    mix1(mix1(a, b, u[0]), mix1(c, d, u[0]), u[1])
}

pub fn fbm(mut p: [f32; 2]) -> f32 {
    let mut a = 0.5f32;
    let mut s = 0.0f32;
    for _ in 0..4 {
        s += a * vnoise(p);
        p = [p[0] * 2.13 + 11.7, p[1] * 2.13 + 5.3];
        a *= 0.5;
    }
    s
}

/// `metalField`, shared verbatim by the three `.slang` passes
/// (chassis_metal / frame_metal / plate_metal). `speck_seed` is the one
/// constant that differs between the three: chassis/frame use `+ 3.1`,
/// plate uses `+ 9.2`.
pub fn metal_field(px: [f32; 2], p: &MetalParams, speck_seed: f32) -> f32 {
    let m = fbm([px[0] * 0.006, px[1] * 0.006]);
    let m2 = fbm([px[0] * 0.02 + 31.4, px[1] * 0.02 + 31.4]);
    let mut tone = 1.0 + p.mottle_amount * (0.9 * (m - 0.5) + 0.5 * (m2 - 0.5));
    let stain = smoothstep(0.60, 0.85, fbm([px[0] * 0.004 + 7.7, px[1] * 0.004 + 7.7]));
    tone -= p.mottle_amount * 0.35 * stain;
    let g = (vnoise([px[0] * 0.9, px[1] * 0.9]) - 0.5) + 0.6 * (hash12(px) - 0.5);
    tone += p.grain_amount * g;
    let streak = vnoise([px[0] * 0.012, px[1] * 0.4]);
    tone += p.scratch_amount * 0.3 * (streak - 0.5);
    let fine = vnoise([px[0] * 0.5 + 3.7, px[1] * 0.05 + 3.7]);
    tone += p.scratch_amount * 0.22 * (fine - 0.5);
    let pit = smoothstep(
        0.78,
        0.95,
        vnoise([px[0] * 0.09 + 13.1, px[1] * 0.09 + 13.1]),
    );
    tone -= p.mottle_amount * 0.3 * pit;
    let speck = step(
        0.9975,
        hash12([
            (px[0] * 0.7).floor() + speck_seed,
            (px[1] * 0.7).floor() + speck_seed,
        ]),
    );
    tone += p.scratch_amount * speck * 1.4;
    tone.max(0.0)
}

fn step(edge: f32, x: f32) -> f32 {
    if x < edge {
        0.0
    } else {
        1.0
    }
}

/// `distortCoordinates`, shared by terminal_frame.frag and frame_metal.frag.
pub fn distort_coordinates(coords: [f32; 2], frame_size: f32, screen_curvature: f32) -> [f32; 2] {
    let padded = [
        coords[0] * (1.0 + frame_size * 2.0) - frame_size,
        coords[1] * (1.0 + frame_size * 2.0) - frame_size,
    ];
    let cc = [padded[0] - 0.5, padded[1] - 0.5];
    let dist = (cc[0] * cc[0] + cc[1] * cc[1]) * screen_curvature;
    [
        padded[0] + cc[0] * (1.0 + dist) * dist,
        padded[1] + cc[1] * (1.0 + dist) * dist,
    ]
}

pub fn rounded_rect_sdf_pixels(
    p: [f32; 2],
    top_left: [f32; 2],
    bottom_right: [f32; 2],
    radius_pixels: f32,
    viewport_size: [f32; 2],
) -> f32 {
    let size_px = [
        (bottom_right[0] - top_left[0]) * viewport_size[0],
        (bottom_right[1] - top_left[1]) * viewport_size[1],
    ];
    let center_px = [
        (top_left[0] + bottom_right[0]) * 0.5 * viewport_size[0],
        (top_left[1] + bottom_right[1]) * 0.5 * viewport_size[1],
    ];
    let local_px = [
        p[0] * viewport_size[0] - center_px[0],
        p[1] * viewport_size[1] - center_px[1],
    ];
    let half_size = [
        size_px[0] * 0.5 - radius_pixels,
        size_px[1] * 0.5 - radius_pixels,
    ];
    let d = [
        (local_px[0].abs() - half_size[0]).max(0.0),
        (local_px[1].abs() - half_size[1]).max(0.0),
    ];
    let inside = (local_px[0].abs() - half_size[0])
        .max(local_px[1].abs() - half_size[1])
        .min(0.0);
    (d[0] * d[0] + d[1] * d[1]).sqrt() + inside - radius_pixels
}

fn normalize2(v: [f32; 2]) -> [f32; 2] {
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if len <= 1e-20 {
        [0.0, 0.0]
    } else {
        [v[0] / len, v[1] / len]
    }
}

fn dot2(a: [f32; 2], b: [f32; 2]) -> f32 {
    a[0] * b[0] + a[1] * b[1]
}

/// `chassis_metal.frag::main`, exact (no `sin`-based noise here beyond
/// `metal_field`'s own hash noise, which is deterministic and does not use
/// `sin`, so this oracle is bit-comparable modulo float rounding).
pub fn chassis_metal(uv: [f32; 2], viewport: [f32; 2], p: &ChassisMetalParams) -> [f32; 3] {
    let static_coords = [
        uv[0] * p.field_scale[0] + p.field_offset[0],
        uv[1] * p.field_scale[1] + p.field_offset[1],
    ];
    let px = [
        static_coords[0] * viewport[0],
        static_coords[1] * viewport[1],
    ];

    let l = normalize2(p.light_dir);
    let light_pos = [
        (0.5 + l[0] * 0.45).clamp(0.0, 1.0),
        (0.5 + l[1] * 0.45).clamp(0.0, 1.0),
    ];
    let away = dot2(
        [
            static_coords[0] - light_pos[0],
            static_coords[1] - light_pos[1],
        ],
        [-l[0], -l[1]],
    )
    .clamp(0.0, 2.0);
    let mut vig = 1.0 - p.vignette_strength * smoothstep(0.0, 1.1, away);
    let corner = [
        (static_coords[0] - 0.5) * 2.0,
        (static_coords[1] - 0.5) * 2.0,
    ];
    let corner_d = (corner[0] * corner[0] + corner[1] * corner[1]).sqrt() / 1.414;
    vig *= 1.0 - p.vignette_strength * 0.55 * smoothstep(0.62, 1.0, corner_d);

    let field = metal_field(px, &p.metal, 3.1);
    [
        p.chassis_color[0] * field * vig,
        p.chassis_color[1] * field * vig,
        p.chassis_color[2] * field * vig,
    ]
}

fn rrect_px(p: [f32; 2], top_left: [f32; 2], bottom_right: [f32; 2], rad: f32) -> f32 {
    let c = [
        (top_left[0] + bottom_right[0]) * 0.5,
        (top_left[1] + bottom_right[1]) * 0.5,
    ];
    let half_size = [
        (bottom_right[0] - top_left[0]) * 0.5 - rad,
        (bottom_right[1] - top_left[1]) * 0.5 - rad,
    ];
    let d = [
        (p[0] - c[0]).abs() - half_size[0],
        (p[1] - c[1]).abs() - half_size[1],
    ];
    let dp = [d[0].max(0.0), d[1].max(0.0)];
    (dp[0] * dp[0] + dp[1] * dp[1]).sqrt() + d[0].max(d[1]).min(0.0) - rad
}

/// `plate_metal.frag::main`, exact (same noise-determinism caveat as
/// `chassis_metal`: no `sin`).
pub fn plate_metal(uv: [f32; 2], p: &PlateMetalParams) -> ([f32; 3], f32) {
    let px = [uv[0] * p.size_px[0], uv[1] * p.size_px[1]];
    let d = rrect_px(
        px,
        [0.5, 0.5],
        [p.size_px[0] - 0.5, p.size_px[1] - 0.5],
        p.corner_radius,
    );
    let coverage = 1.0 - smoothstep(-1.0, 0.5, d);

    let uvn = uv;
    let l = normalize2(p.light_dir);

    let field = metal_field(
        [px[0] + p.seed * 137.0, px[1] + p.seed * 61.0],
        &p.metal,
        9.2,
    );
    let mut col = [
        p.base_color[0] * field,
        p.base_color[1] * field,
        p.base_color[2] * field,
    ];

    let blotch = smoothstep(
        0.60,
        0.92,
        fbm([px[0] * 0.008 + p.seed * 31.0, px[1] * 0.008 + p.seed * 31.0]),
    );
    let t = p.wear_amount * 0.55 * blotch;
    col = [
        mix1(col[0], p.highlight_color[0] * 0.42, t),
        mix1(col[1], p.highlight_color[1] * 0.42, t),
        mix1(col[2], p.highlight_color[2] * 0.42, t),
    ];

    let edge_dist = -d;
    let near_edge = smoothstep(p.bevel_px + 6.0, 0.0, edge_dist);
    let wear_noise = fbm([px[0] * 0.03 + p.seed * 17.0, px[1] * 0.03 + p.seed * 17.0]);
    let t2 = p.wear_amount * near_edge * smoothstep(0.35, 0.8, wear_noise);
    let hl = [
        p.highlight_color[0] * (0.45 + 0.8 * wear_noise),
        p.highlight_color[1] * (0.45 + 0.8 * wear_noise),
        p.highlight_color[2] * (0.45 + 0.8 * wear_noise),
    ];
    col = [
        mix1(col[0], hl[0], t2),
        mix1(col[1], hl[1], t2),
        mix1(col[2], hl[2], t2),
    ];

    let bt = 1.0 - smoothstep(0.0, p.bevel_px, px[1]);
    let bl = 1.0 - smoothstep(0.0, p.bevel_px, px[0]);
    let br = 1.0 - smoothstep(0.0, p.bevel_px, p.size_px[0] - px[0]);
    let bb = 1.0 - smoothstep(0.0, p.bevel_px, p.size_px[1] - px[1]);
    let lit = bt * (-l[1]).clamp(0.0, 1.0)
        + bl * (-l[0]).clamp(0.0, 1.0)
        + br * l[0].clamp(0.0, 1.0)
        + bb * l[1].clamp(0.0, 1.0);
    let shad = bt * l[1].clamp(0.0, 1.0)
        + bl * l[0].clamp(0.0, 1.0)
        + br * (-l[0]).clamp(0.0, 1.0)
        + bb * (-l[1]).clamp(0.0, 1.0);
    let lit_c = lit.clamp(0.0, 1.0);
    col = [
        col[0] + p.highlight_color[0] * lit_c * 0.9,
        col[1] + p.highlight_color[1] * lit_c * 0.9,
        col[2] + p.highlight_color[2] * lit_c * 0.9,
    ];
    let shad_c = shad.clamp(0.0, 1.0) * 0.7;
    col = [
        mix1(col[0], p.shadow_color[0], shad_c),
        mix1(col[1], p.shadow_color[1], shad_c),
        mix1(col[2], p.shadow_color[2], shad_c),
    ];

    let seam = 1.0 - smoothstep(0.8, 2.8, (edge_dist - (p.bevel_px + 2.0)).abs());
    let seam_mul = 1.0 - 0.35 * p.seam_gain * seam;
    col = [col[0] * seam_mul, col[1] * seam_mul, col[2] * seam_mul];

    let light_pos = [
        (0.5 + l[0] * 0.5).clamp(0.0, 1.0),
        (0.5 + l[1] * 0.5).clamp(0.0, 1.0),
    ];
    let away = dot2(
        [uvn[0] - light_pos[0], uvn[1] - light_pos[1]],
        [-l[0], -l[1]],
    )
    .clamp(0.0, 2.0);
    let mut vig = 1.0 - p.vignette_strength * smoothstep(0.0, 1.2, away);
    let corner = [(uvn[0] - 0.5) * 2.0, (uvn[1] - 0.5) * 2.0];
    let corner_d = (corner[0] * corner[0] + corner[1] * corner[1]).sqrt() / 1.414;
    vig *= 1.0 - p.vignette_strength * 0.5 * smoothstep(0.6, 1.0, corner_d);
    col = [col[0] * vig, col[1] * vig, col[2] * vig];

    (col, coverage)
}

/// `frame_metal.frag::main`, exact (no `sin` anywhere in this shader either).
pub fn frame_metal(uv: [f32; 2], viewport: [f32; 2], p: &FrameMetalParams) -> ([f32; 3], f32) {
    let static_coords = uv;
    let coords = distort_coordinates(static_coords, p.frame_size, p.screen_curvature);
    let px = [
        static_coords[0] * viewport[0],
        static_coords[1] * viewport[1],
    ];

    let dist_px =
        rounded_rect_sdf_pixels(coords, [0.0, 0.0], [1.0, 1.0], p.screen_radius, viewport);
    let d_outer = rrect_px(
        px,
        [p.bezel_margins[0], p.bezel_margins[1]],
        [
            viewport[0] - p.bezel_margins[2],
            viewport[1] - p.bezel_margins[3],
        ],
        p.outer_radius,
    );

    let cc = [coords[0] - 0.5, coords[1] - 0.5];
    let nrm = normalize2([cc[0] + 1e-5, cc[1] + 1e-5]);
    let l = normalize2(p.light_dir);

    let light_pos = [
        (0.5 + l[0] * 0.45).clamp(0.0, 1.0),
        (0.5 + l[1] * 0.45).clamp(0.0, 1.0),
    ];
    let away = dot2(
        [
            static_coords[0] - light_pos[0],
            static_coords[1] - light_pos[1],
        ],
        [-l[0], -l[1]],
    )
    .clamp(0.0, 2.0);
    let mut vig = 1.0 - p.vignette_strength * smoothstep(0.0, 1.1, away);
    let corner = [
        (static_coords[0] - 0.5) * 2.0,
        (static_coords[1] - 0.5) * 2.0,
    ];
    let corner_d = (corner[0] * corner[0] + corner[1] * corner[1]).sqrt() / 1.414;
    vig *= 1.0 - p.vignette_strength * 0.55 * smoothstep(0.62, 1.0, corner_d);

    let surf = metal_field(px, &p.metal, 3.1);

    let band = 0.55 + p.fill_gain * dot2(nrm, l).clamp(0.0, 1.0);
    let mut bez = [
        p.bezel_color[0] * band * surf,
        p.bezel_color[1] * band * surf,
        p.bezel_color[2] * band * surf,
    ];
    let mut well = mix1(1.0, p.well_floor, smoothstep(p.well_depth, 0.0, dist_px));
    if p.face_band_px > 0.5 {
        let wall = mix1(
            1.0,
            p.well_floor,
            smoothstep(p.face_band_px * 0.5, p.face_band_px * 1.8, -d_outer),
        );
        well = well.min(wall);
    }
    bez = [bez[0] * well, bez[1] * well, bez[2] * well];

    let trough = smoothstep(p.well_depth * 0.35, p.well_depth * 0.9, dist_px)
        * (1.0 - smoothstep(p.well_depth * 0.9, p.well_depth * 1.7, dist_px));
    let trough_factor =
        surf * trough * p.trough_gain * (0.4 + 0.9 * (-dot2(nrm, l)).clamp(0.0, 1.0));
    bez = [
        bez[0] + p.bezel_color[0] * trough_factor,
        bez[1] + p.bezel_color[1] * trough_factor,
        bez[2] + p.bezel_color[2] * trough_factor,
    ];

    let standing_rim = p.rim_gain > 0.001;
    let facing = (-dot2(nrm, l)).clamp(0.0, 1.0);
    let rim_d = if standing_rim {
        (-d_outer - p.rim_dist_px).abs()
    } else {
        (dist_px - 2.0).abs()
    };
    let rim_w = if standing_rim { 3.0 } else { 2.5 };
    let lip = (1.0 - smoothstep(0.0, rim_w, rim_d))
        * if standing_rim {
            0.45 + 0.55 * facing
        } else {
            facing
        };
    let lip_gain = lip
        * if standing_rim {
            p.rim_gain
        } else {
            0.4 * p.ridge_gain
        };
    bez = [
        bez[0] + p.ridge_color[0] * lip_gain,
        bez[1] + p.ridge_color[1] * lip_gain,
        bez[2] + p.ridge_color[2] * lip_gain,
    ];

    let cha = [
        p.chassis_color[0] * surf,
        p.chassis_color[1] * surf,
        p.chassis_color[2] * surf,
    ];

    let in_bezel = smoothstep(1.0, -1.0, d_outer);
    let mut metal = [
        mix1(cha[0], bez[0], in_bezel),
        mix1(cha[1], bez[1], in_bezel),
        mix1(cha[2], bez[2], in_bezel),
    ];

    let groove = 1.0 - smoothstep(0.0, 2.2, (d_outer - 1.8).abs());
    let groove_mul = 1.0 - 0.6 * groove;
    metal = [
        metal[0] * groove_mul,
        metal[1] * groove_mul,
        metal[2] * groove_mul,
    ];
    let ridge_line = 1.0 - smoothstep(0.5, 2.4, (d_outer + 2.0).abs());
    let ridge_lit = 0.45 + 0.55 * dot2(nrm, l).clamp(0.0, 1.0);
    let ridge_add = ridge_line * ridge_lit * p.ridge_gain;
    metal = [
        metal[0] + p.ridge_color[0] * ridge_add,
        metal[1] + p.ridge_color[1] * ridge_add,
        metal[2] + p.ridge_color[2] * ridge_add,
    ];

    metal = [metal[0] * vig, metal[1] * vig, metal[2] * vig];

    let in_screen = smoothstep(0.0, 1.0, -dist_px);
    let frame_alpha = 1.0 - p.frame_shininess * 0.4;
    let alpha = mix1(frame_alpha, mix1(0.0, 0.3, p.ambient_light), in_screen);
    let glass = (p.ambient_light
        * (coords[0] * (1.0 - coords[1]) * coords[1] * (1.0 - coords[0]) * 25.0)
            .max(0.0)
            .powf(0.5)
        * in_screen)
        .clamp(0.0, 1.0);
    let color = [
        mix1(metal[0], glass, in_screen),
        mix1(metal[1], glass, in_screen),
        mix1(metal[2], glass, in_screen),
    ];

    (color, alpha)
}


pub struct TerminalFrameParams {
    pub screen_curvature: f32,
    pub frame_color: [f32; 4],
    pub frame_size: f32,
    pub screen_radius: f32,
    pub ambient_light: f32,
    pub frame_shininess: f32,
}

/// `terminal_frame.slang::main`, minus `noise` (a `sin`-based hash the CPU and
/// GPU will not agree on bit-for-bit; the test that uses this oracle checks
/// the noise-free channels and gives the noise-bearing ones a wider band).
pub fn terminal_frame(
    uv: [f32; 2],
    viewport: [f32; 2],
    p: &TerminalFrameParams,
) -> ([f32; 3], f32) {
    let coords = distort_coordinates(uv, p.frame_size, p.screen_curvature);
    let screen_radius_px = p.screen_radius;
    let edge_soft_px = 1.0f32;
    let seam_width = screen_radius_px.max(0.5) / viewport[0].min(viewport[1]);

    let e = smoothstep(-seam_width, seam_width, coords[0] - coords[1]).min(smoothstep(
        -seam_width,
        seam_width,
        coords[0] - (1.0 - coords[1]),
    ));
    let s = smoothstep(-seam_width, seam_width, coords[1] - coords[0]).min(smoothstep(
        -seam_width,
        seam_width,
        coords[0] - (1.0 - coords[1]),
    ));
    let w = smoothstep(-seam_width, seam_width, coords[1] - coords[0]).min(smoothstep(
        -seam_width,
        seam_width,
        (1.0 - coords[0]) - coords[1],
    ));
    let n = smoothstep(-seam_width, seam_width, coords[0] - coords[1]).min(smoothstep(
        -seam_width,
        seam_width,
        (1.0 - coords[0]) - coords[1],
    ));

    let dist_px =
        rounded_rect_sdf_pixels(coords, [0.0, 0.0], [1.0, 1.0], screen_radius_px, viewport);
    let mut frame_shadow = e * 0.66 + w * 0.66 + n * 0.33 + s;
    frame_shadow *= smoothstep(0.0, edge_soft_px * 5.0, dist_px);

    let frame_alpha = 1.0 - p.frame_shininess * 0.4;
    let in_screen = smoothstep(0.0, edge_soft_px, -dist_px);
    let alpha = mix1(frame_alpha, mix1(0.0, 0.3, p.ambient_light), in_screen);
    let glass = (p.ambient_light
        * (coords[0] * (1.0 - coords[1]) * coords[1] * (1.0 - coords[0]) * 25.0)
            .max(0.0)
            .powf(0.5)
        * in_screen)
        .clamp(0.0, 1.0);
    let frame_tint = [
        p.frame_color[0] * frame_shadow,
        p.frame_color[1] * frame_shadow,
        p.frame_color[2] * frame_shadow,
    ];
    let color = [
        mix1(frame_tint[0], glass, in_screen),
        mix1(frame_tint[1], glass, in_screen),
        mix1(frame_tint[2], glass, in_screen),
    ];
    (color, alpha)
}



/// The bloom passes' kernel (`bloom_h.slang` / `bloom_v.slang`) as a
/// definition rather than as their tap loop: a Gaussian of `sigma = radius /
/// 3`, truncated at `+/-radius`, integrated against the 1-D input profile
/// `sample_fn` (offset in uv units, `texel` the uv size of one output pixel)
/// by quadrature sixteen steps to the texel. Returns the blurred value at
/// offset 0. A shader whose taps sit no more than one texel apart converges
/// on this whatever its stride; one whose taps sit wider does not, which is
/// what makes it a check on the shader rather than a mirror of it.
pub fn gaussian_blur_1d(radius: f32, texel: f32, sample_fn: impl Fn(f32) -> f32) -> f32 {
    const STEPS_PER_TEXEL: i32 = 16;
    let sigma = (radius / 3.0).max(1e-4);
    let n = (radius * STEPS_PER_TEXEL as f32).ceil() as i32;
    let mut sum = 0.0f32;
    let mut wsum = 0.0f32;
    for i in -n..=n {
        let x = i as f32 / STEPS_PER_TEXEL as f32;
        let w = (-0.5 * (x * x) / (sigma * sigma)).exp();
        sum += sample_fn(x * texel) * w;
        wsum += w;
    }
    sum / wsum.max(1e-6)
}
