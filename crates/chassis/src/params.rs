//! The uniform payloads of the procedural metal shaders: what a host (a
//! shell's furniture, the app's chrome) fills in to draw a metal, and what
//! the shader-oracle test crate's CPU reimplementations take as input.
//!
//! The `*_record` functions below are the other half: the parameter blocks
//! the WGSL bodies under `shaders/wgsl/` declare, in the field order and with
//! the padding those structs' layouts put there. One statement of each
//! layout, read by the mount that fills the buffer and by the test that
//! measures the shader against the CPU oracle.

use crate::frame::Param;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MetalParams {
    pub grain_amount: f32,
    pub mottle_amount: f32,
    pub scratch_amount: f32,
}


#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChassisMetalParams {
    pub field_scale: [f32; 2],
    pub field_offset: [f32; 2],
    pub light_dir: [f32; 2],
    pub chassis_color: [f32; 3],
    pub metal: MetalParams,
    pub vignette_strength: f32,
}

/// The float count of [`ChassisMetalParams::record`], which is the WGSL
/// `ChassisParams` struct's size in floats.
pub const CHASSIS_RECORD_FLOATS: usize = 16;

impl ChassisMetalParams {
    /// The `ChassisParams` block `shaders/wgsl/chassis_metal.wgsl` declares,
    /// in the field order and with the padding that struct's layout puts
    /// there: four `vec2`s, a `vec4`, four scalars.
    ///
    /// `viewport_size` is the screen well in **logical** pixels and is the
    /// host's to supply, because it is the field this casting is measured in
    /// rather than the rectangle it covers.
    ///
    /// One statement of the layout, read by the mount that fills the buffer
    /// and by the test that measures the shader against the CPU oracle. The
    /// alpha the slang pass carried is gone: the casting is opaque and the
    /// uniform was never read.
    pub fn record(&self, viewport_size: [f32; 2]) -> [f32; CHASSIS_RECORD_FLOATS] {
        [
            self.field_scale[0],
            self.field_scale[1],
            self.field_offset[0],
            self.field_offset[1],
            self.light_dir[0],
            self.light_dir[1],
            viewport_size[0],
            viewport_size[1],
            self.chassis_color[0],
            self.chassis_color[1],
            self.chassis_color[2],
            1.0,
            self.metal.grain_amount,
            self.metal.mottle_amount,
            self.metal.scratch_amount,
            self.vignette_strength,
        ]
    }
}


pub struct PlateMetalParams {
    pub size_px: [f32; 2],
    pub light_dir: [f32; 2],
    pub base_color: [f32; 3],
    pub highlight_color: [f32; 3],
    pub shadow_color: [f32; 3],
    pub corner_radius: f32,
    pub bevel_px: f32,
    pub metal: MetalParams,
    pub vignette_strength: f32,
    pub wear_amount: f32,
    pub seam_gain: f32,
    pub seed: f32,
}


pub struct FrameMetalParams {
    pub screen_curvature: f32,
    pub frame_size: f32,
    pub screen_radius: f32,
    pub ambient_light: f32,
    pub frame_shininess: f32,
    pub light_dir: [f32; 2],
    pub bezel_color: [f32; 3],
    pub chassis_color: [f32; 3],
    pub ridge_color: [f32; 3],
    pub bezel_margins: [f32; 4],
    pub outer_radius: f32,
    pub well_depth: f32,
    pub well_floor: f32,
    pub ridge_gain: f32,
    pub metal: MetalParams,
    pub vignette_strength: f32,
    pub fill_gain: f32,
    pub trough_gain: f32,
    pub face_band_px: f32,
    pub rim_dist_px: f32,
    pub rim_gain: f32,
}


/// The float count of a furniture piece's parameter record, which is the
/// size in floats of the `PlateParams`, `LedParams` and `TapeParams` structs
/// the three WGSL bodies under `shaders/wgsl/` declare. They are one size on
/// purpose: a mount holds one buffer per kind and a rig binds one block, and
/// a single number is one thing to keep true.
pub const PIECE_RECORD_FLOATS: usize = 28;

/// One named uniform out of a piece's list, or zero.
///
/// Zero rather than a panic because a missing name is a mount that has not
/// been finished, not a corrupt one, and the tests below name every field
/// each record wants.
fn named(params: &[Param], name: &str) -> f32 {
    for (n, v) in params {
        if *n == name {
            return *v;
        }
    }
    0.0
}

/// The `PlateParams` block `shaders/wgsl/plate_metal.wgsl` declares, from the
/// named list [`crate::furniture::plate_params`] builds.
pub fn plate_record(p: &[Param]) -> [f32; PIECE_RECORD_FLOATS] {
    [
        named(p, "sizePxX"),
        named(p, "sizePxY"),
        named(p, "lightDirX"),
        named(p, "lightDirY"),
        named(p, "baseColorR"),
        named(p, "baseColorG"),
        named(p, "baseColorB"),
        named(p, "baseColorA"),
        named(p, "highlightColorR"),
        named(p, "highlightColorG"),
        named(p, "highlightColorB"),
        named(p, "highlightColorA"),
        named(p, "shadowColorR"),
        named(p, "shadowColorG"),
        named(p, "shadowColorB"),
        named(p, "shadowColorA"),
        named(p, "cornerRadius"),
        named(p, "bevelPx"),
        named(p, "grainAmount"),
        named(p, "mottleAmount"),
        named(p, "scratchAmount"),
        named(p, "vignetteStrength"),
        named(p, "wearAmount"),
        named(p, "seamGain"),
        named(p, "seed"),
        0.0,
        0.0,
        0.0,
    ]
}

/// The `LedParams` block `shaders/wgsl/led_matrix.wgsl` declares, from the
/// named list [`crate::furniture::led_params`] builds.
///
/// `atlas` is where this strip's lamp raster sits in the host's atlas, as
/// origin and extent in the atlas's own 0..1 coordinates. It is the host's to
/// supply: the shader asks for its raster in the raster's own coordinates and
/// the mount decides where that lives.
pub fn led_record(p: &[Param], atlas: [f32; 4]) -> [f32; PIECE_RECORD_FLOATS] {
    [
        named(p, "gridSizeX"),
        named(p, "gridSizeY"),
        named(p, "spillMarginX"),
        named(p, "spillMarginY"),
        named(p, "spillDeadX"),
        named(p, "spillDeadY"),
        0.0,
        0.0,
        atlas[0],
        atlas[1],
        atlas[2],
        atlas[3],
        named(p, "litColorR"),
        named(p, "litColorG"),
        named(p, "litColorB"),
        named(p, "litColorA"),
        named(p, "dimColorR"),
        named(p, "dimColorG"),
        named(p, "dimColorB"),
        named(p, "dimColorA"),
        named(p, "panelColorR"),
        named(p, "panelColorG"),
        named(p, "panelColorB"),
        named(p, "panelColorA"),
        named(p, "dotRadius"),
        named(p, "threshold"),
        named(p, "glow"),
        named(p, "spillStrength"),
    ]
}

/// The `TapeParams` block `shaders/wgsl/tape_label.wgsl` declares, from the
/// named list [`crate::furniture::tape_params`] builds. `atlas` is as
/// [`led_record`]'s.
pub fn tape_record(p: &[Param], atlas: [f32; 4]) -> [f32; PIECE_RECORD_FLOATS] {
    [
        named(p, "sizePxX"),
        named(p, "sizePxY"),
        named(p, "lightDirX"),
        named(p, "lightDirY"),
        named(p, "glyphRectPxX"),
        named(p, "glyphRectPxY"),
        named(p, "glyphRectPxZ"),
        named(p, "glyphRectPxW"),
        atlas[0],
        atlas[1],
        atlas[2],
        atlas[3],
        named(p, "tapeColorR"),
        named(p, "tapeColorG"),
        named(p, "tapeColorB"),
        named(p, "tapeColorA"),
        named(p, "letterColorR"),
        named(p, "letterColorG"),
        named(p, "letterColorB"),
        named(p, "letterColorA"),
        named(p, "bevelPx"),
        named(p, "dilatePx"),
        named(p, "sheenAmount"),
        named(p, "grainAmount"),
        named(p, "seed"),
        0.0,
        0.0,
        0.0,
    ]
}

/// A record's bytes, for a buffer write.
pub fn record_bytes(record: &[f32; PIECE_RECORD_FLOATS]) -> Vec<u8> {
    record.iter().flat_map(|f| f.to_ne_bytes()).collect()
}
