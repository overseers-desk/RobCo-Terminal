// The furniture that is drawn rather than shaded: rounded rectangles with
// gradients, stroked arcs, filled polygons, and lines of text.
//
// One [`chassis::paint::Op`] is one instance here. The host gives every
// operation its own rectangle and its own record, in the order the painting
// lists them, so the fixed-function blender composites them source-over in
// exactly the order a single accumulator would have.
//
// Every measure in a record is in **device** pixels of the piece the
// operation belongs to: the host multiplies the furniture's logical
// coordinates by the window's ratio on the way in, and `origin` is where
// that piece's own (0, 0) sits on the target, so `p = position - origin` puts
// a fragment back in the painting's coordinates. Keeping the arithmetic
// piece-local rather than target-absolute is what keeps a radial gradient's
// two-circle solve inside f32's reach on a tall column.
//
// Antialiasing is a signed distance field: the exact distance to the
// boundary taken to coverage as `clamp(0.5 - d, 0, 1)`, which is exact for
// the axis-aligned edges most of this furniture is made of. The polygon is
// the exception and supersamples, because a straight-edged path has no
// closed-form distance the way a rounded rectangle does.
//
// Text is the other exception: the glyphs are struck on the CPU through
// swash, packed into the host's atlas, and read here as coverage. The
// coverage is one number per pixel, the largest of the three subpixel
// channels, so a line composites on a single alpha.
//
// Requires `common.wgsl` ahead of it, and a host that declares the `stops`
// and `points` storage arrays and the `chrome_sample` atlas read. This file
// declares no bindings and no entry points.

struct VectorParams {
    // The shape's rectangle: origin then extent, piece-local device pixels.
    // For a text run it is where the struck raster lands.
    rect: vec4<f32>,
    // A parent's clip rectangle, read in the parent's own unrotated frame.
    clip: vec4<f32>,
    color: vec4<f32>,
    border_color: vec4<f32>,
    // A radial gradient's inner circle as (x, y, r); an arc's centre,
    // radius and line width.
    g0: vec4<f32>,
    // A radial gradient's outer circle as (x, y, r); an arc's start and end
    // angle.
    g1: vec4<f32>,
    // Where a text run's raster sits in the host's atlas, as origin and
    // extent in the atlas's own 0..1 coordinates.
    atlas: vec4<f32>,
    // Where the piece's own (0, 0) sits on the target, in its pixels.
    origin: vec2<f32>,
    radius: f32,
    border_width: f32,
    clip_radius: f32,
    opacity: f32,
    rotation: f32,
    pivot_x: f32,
    pivot_y: f32,
    // 1 where a linear gradient runs left to right instead of top to bottom.
    horizontal: f32,
    // Where this shape's run of gradient stops or polygon points begins, and
    // how many it has.
    span_offset: u32,
    span_count: u32,
    has_clip: u32,
    // A struct with a `vec4` field aligns to 16, so its array stride rounds
    // up to it; these three carry that rounding in the declaration.
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

struct GradStop {
    color: vec4<f32>,
    position: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

// The exact distance from a point to a rounded rectangle's boundary,
// negative inside.
fn vector_rrect_distance(p: vec2<f32>, rect: vec4<f32>, radius: f32) -> f32 {
    let h = rect.zw * 0.5;
    let r = max(min(radius, min(h.x, h.y)), 0.0);
    let q = abs(p - (rect.xy + h)) - (h - vec2<f32>(r, r));
    return length(max(q, vec2<f32>(0.0, 0.0))) + min(max(q.x, q.y), 0.0) - r;
}

fn vector_coverage(distance: f32) -> f32 {
    return clamp(0.5 - distance, 0.0, 1.0);
}

// Source-over of a straight colour at a coverage onto a premultiplied
// accumulator.
fn vector_over(dst: vec4<f32>, color: vec4<f32>, alpha: f32) -> vec4<f32> {
    if (alpha <= 0.0) {
        return dst;
    }
    let a = clamp(alpha, 0.0, 1.0);
    let src = vec4<f32>(color.rgb * a, a);
    return src + dst * (1.0 - a);
}

// A gradient's colour at `t`, with a pad spread at both ends and straight
// interpolation between neighbouring stops.
fn vector_stop_color(offset: u32, count: u32, t: f32) -> vec4<f32> {
    if (count == 0u) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let first = stops[offset];
    if (t <= first.position) {
        return first.color;
    }
    let last = stops[offset + count - 1u];
    if (t >= last.position) {
        return last.color;
    }
    for (var i = 0u; i + 1u < count; i = i + 1u) {
        let a = stops[offset + i];
        let b = stops[offset + i + 1u];
        if (t >= a.position && t <= b.position) {
            let span = b.position - a.position;
            var f = 0.0;
            if (span > 0.0) {
                f = (t - a.position) / span;
            }
            return a.color + (b.color - a.color) * f;
        }
    }
    return last.color;
}

// The canvas two-circle radial gradient's parameter at a point: the largest
// `t` whose interpolated circle passes through it, padded outside 0..1.
//
// Both roots, and the greater of the two that names a circle with a real
// radius. Which of `(b +- sqrt) / a` that is depends on the sign of `a`, and
// `a` is negative for every concentric gradient, which is all but one of the
// screw head's.
fn vector_radial_t(inner: vec3<f32>, outer: vec3<f32>, p: vec2<f32>) -> f32 {
    let cd = outer.xy - inner.xy;
    let dr = outer.z - inner.z;
    let pd = p - inner.xy;
    let a = dot(cd, cd) - dr * dr;
    let b = dot(pd, cd) + inner.z * dr;
    let c = dot(pd, pd) - inner.z * inner.z;
    if (abs(a) < 1e-12) {
        if (abs(b) < 1e-12) {
            return 0.0;
        }
        return c / (2.0 * b);
    }
    let disc = b * b - a * c;
    if (disc < 0.0) {
        return 0.0;
    }
    let root = sqrt(disc);
    let hi = max((b + root) / a, (b - root) / a);
    let lo = min((b + root) / a, (b - root) / a);
    if (inner.z + hi * dr >= 0.0) {
        return hi;
    }
    if (inner.z + lo * dr >= 0.0) {
        return lo;
    }
    return 0.0;
}

// A rounded rectangle, at a solid fill (`mode` 0), a linear gradient (1) or
// a two-circle radial one (2).
//
// The fill covers the whole rectangle and the border paints inside the same
// bounds on top of it, so the fill runs underneath the border rather than
// meeting it.
fn vector_rect(position: vec2<f32>, v: VectorParams, mode: u32) -> vec4<f32> {
    var p = position - v.origin;
    var clip_cov = 1.0;
    if (v.has_clip == 1u) {
        clip_cov = vector_coverage(vector_rrect_distance(p, v.clip, v.clip_radius));
    }
    if (clip_cov <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    if (v.rotation != 0.0) {
        let s = sin(-v.rotation);
        let c = cos(-v.rotation);
        let pivot = vec2<f32>(v.pivot_x, v.pivot_y);
        let d = p - pivot;
        p = pivot + vec2<f32>(d.x * c - d.y * s, d.x * s + d.y * c);
    }
    let cov = vector_coverage(vector_rrect_distance(p, v.rect, v.radius)) * clip_cov;
    if (cov <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }

    var colour = v.color;
    if (mode == 1u) {
        var t = 0.0;
        if (v.horizontal > 0.5) {
            if (v.rect.z > 0.0) {
                t = (p.x - v.rect.x) / v.rect.z;
            }
        } else {
            if (v.rect.w > 0.0) {
                t = (p.y - v.rect.y) / v.rect.w;
            }
        }
        colour = vector_stop_color(v.span_offset, v.span_count, t);
    } else if (mode == 2u) {
        colour = vector_stop_color(
            v.span_offset,
            v.span_count,
            vector_radial_t(v.g0.xyz, v.g1.xyz, p),
        );
    }

    var out = vector_over(
        vec4<f32>(0.0, 0.0, 0.0, 0.0),
        colour,
        cov * v.opacity * colour.a,
    );
    if (v.border_width > 0.0) {
        let bw = v.border_width;
        let inner = vec4<f32>(
            v.rect.x + bw,
            v.rect.y + bw,
            max(v.rect.z - 2.0 * bw, 0.0),
            max(v.rect.w - 2.0 * bw, 0.0),
        );
        let ring = max(
            cov - vector_coverage(vector_rrect_distance(p, inner, v.radius - bw)) * clip_cov,
            0.0,
        );
        out = vector_over(out, v.border_color, ring * v.opacity * v.border_color.a);
    }
    return out;
}

// A stroked arc, butt caps: a band of the line's width about the radius,
// gated hard at both ends of the sweep as a stroke's ends are.
fn vector_arc(position: vec2<f32>, v: VectorParams) -> vec4<f32> {
    let p = position - v.origin;
    let centre = v.g0.xy;
    let radius = v.g0.z;
    let half = v.g0.w * 0.5;
    let d = p - centre;
    let cov = vector_coverage(abs(length(d) - radius) - half);
    if (cov <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let start = v.g1.x;
    let sweep = v.g1.y - start;
    let two_pi = 6.283185307179586;
    var from_start = atan2(d.y, d.x) - start;
    from_start = from_start - floor(from_start / two_pi) * two_pi;
    if (sweep < 0.0) {
        from_start = two_pi - from_start;
    }
    if (from_start > abs(sweep)) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    return vector_over(
        vec4<f32>(0.0, 0.0, 0.0, 0.0),
        v.color,
        cov * v.color.a,
    );
}

// The nonzero winding number of a closed path around a point.
fn vector_winding(offset: u32, count: u32, p: vec2<f32>) -> i32 {
    var n = 0;
    for (var i = 0u; i < count; i = i + 1u) {
        let a = points[offset + i];
        var next = i + 1u;
        if (next == count) {
            next = 0u;
        }
        let b = points[offset + next];
        let cross = (b.x - a.x) * (p.y - a.y) - (p.x - a.x) * (b.y - a.y);
        if (a.y <= p.y) {
            if (b.y > p.y && cross > 0.0) {
                n = n + 1;
            }
        } else if (b.y <= p.y && cross < 0.0) {
            n = n - 1;
        }
    }
    return n;
}

// A filled path. Straight edges have no closed-form distance, so this one is
// supersampled: a 4x4 grid inside the pixel, nonzero winding at each sample.
fn vector_polygon(position: vec2<f32>, v: VectorParams) -> vec4<f32> {
    if (v.span_count < 3u) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let p = position - v.origin;
    let corner = floor(p);
    var hits = 0;
    for (var sy = 0; sy < 4; sy = sy + 1) {
        for (var sx = 0; sx < 4; sx = sx + 1) {
            let s = corner + vec2<f32>((f32(sx) + 0.5) / 4.0, (f32(sy) + 0.5) / 4.0);
            if (vector_winding(v.span_offset, v.span_count, s) != 0) {
                hits = hits + 1;
            }
        }
    }
    if (hits == 0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let cov = f32(hits) / 16.0;
    return vector_over(
        vec4<f32>(0.0, 0.0, 0.0, 0.0),
        v.color,
        cov * v.opacity * v.color.a,
    );
}

// One line of text, out of the host's atlas at one texel per device pixel.
fn vector_text(position: vec2<f32>, v: VectorParams) -> vec4<f32> {
    let p = position - v.origin;
    if (v.rect.z <= 0.0 || v.rect.w <= 0.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let uv = (p - v.rect.xy) / v.rect.zw;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let cov = chrome_sample(uv, v.atlas).a;
    return vector_over(
        vec4<f32>(0.0, 0.0, 0.0, 0.0),
        v.color,
        cov * v.opacity * v.color.a,
    );
}
