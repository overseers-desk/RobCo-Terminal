//! The uniform payloads of the procedural metal shaders: what a host (a
//! shell's furniture, the app's chrome) fills in to draw a metal, and what
//! the shader-oracle test crate's CPU reimplementations take as input.
//!
//! The `record` methods below are the other half: the parameter blocks the
//! WGSL bodies under `shaders/wgsl/` declare, in the field order and with the
//! padding those structs' layouts put there. One statement of each layout,
//! read by the mount that fills the buffer and by the test that measures the
//! shader against the CPU oracle.

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


#[derive(Clone, Copy, Debug, PartialEq)]
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

impl PlateMetalParams {
    /// The `PlateParams` block `shaders/wgsl/plate_metal.wgsl` declares, in
    /// that struct's own field order: two `vec2`s, three `vec4` colours, then
    /// the scalars and the tail padding the std430 stride puts there.
    ///
    /// The three colours are opaque; the shader multiplies coverage in
    /// itself, so the alpha a caller could pass would only ever be one.
    pub fn record(&self) -> [f32; PIECE_RECORD_FLOATS] {
        [
            self.size_px[0],
            self.size_px[1],
            self.light_dir[0],
            self.light_dir[1],
            self.base_color[0],
            self.base_color[1],
            self.base_color[2],
            1.0,
            self.highlight_color[0],
            self.highlight_color[1],
            self.highlight_color[2],
            1.0,
            self.shadow_color[0],
            self.shadow_color[1],
            self.shadow_color[2],
            1.0,
            self.corner_radius,
            self.bevel_px,
            self.metal.grain_amount,
            self.metal.mottle_amount,
            self.metal.scratch_amount,
            self.vignette_strength,
            self.wear_amount,
            self.seam_gain,
            self.seed,
            0.0,
            0.0,
            0.0,
        ]
    }
}


/// `led_matrix.wgsl`'s uniforms for one window of lamps.
///
/// The two spill fields are fractions of the drawn rectangle, not pixels:
/// the rectangle stands proud of the lamp grid by the spill margin on each
/// side, and the shader remaps its texcoords by that fraction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LedMetalParams {
    pub grid_size: [f32; 2],
    pub spill_margin: [f32; 2],
    pub spill_dead: [f32; 2],
    pub lit_color: [f32; 4],
    pub dim_color: [f32; 4],
    pub panel_color: [f32; 4],
    pub dot_radius: f32,
    pub threshold: f32,
    pub glow: f32,
    pub spill_strength: f32,
}

impl LedMetalParams {
    /// The `LedParams` block `shaders/wgsl/led_matrix.wgsl` declares, in that
    /// struct's own field order.
    ///
    /// `atlas` is where this strip's lamp raster sits in the host's atlas, as
    /// origin and extent in the atlas's own 0..1 coordinates. It is the
    /// host's to supply: the shader asks for its raster in the raster's own
    /// coordinates and the mount decides where that lives.
    pub fn record(&self, atlas: [f32; 4]) -> [f32; PIECE_RECORD_FLOATS] {
        [
            self.grid_size[0],
            self.grid_size[1],
            self.spill_margin[0],
            self.spill_margin[1],
            self.spill_dead[0],
            self.spill_dead[1],
            0.0,
            0.0,
            atlas[0],
            atlas[1],
            atlas[2],
            atlas[3],
            self.lit_color[0],
            self.lit_color[1],
            self.lit_color[2],
            self.lit_color[3],
            self.dim_color[0],
            self.dim_color[1],
            self.dim_color[2],
            self.dim_color[3],
            self.panel_color[0],
            self.panel_color[1],
            self.panel_color[2],
            self.panel_color[3],
            self.dot_radius,
            self.threshold,
            self.glow,
            self.spill_strength,
        ]
    }
}


/// `tape_label.wgsl`'s uniforms for one stamped label.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TapeMetalParams {
    pub size_px: [f32; 2],
    pub light_dir: [f32; 2],
    pub glyph_rect_px: [f32; 4],
    pub tape_color: [f32; 4],
    pub letter_color: [f32; 4],
    pub bevel_px: f32,
    pub dilate_px: f32,
    pub sheen_amount: f32,
    pub grain_amount: f32,
    pub seed: f32,
}

impl TapeMetalParams {
    /// The `TapeParams` block `shaders/wgsl/tape_label.wgsl` declares, in that
    /// struct's own field order. `atlas` is as [`LedMetalParams::record`]'s.
    pub fn record(&self, atlas: [f32; 4]) -> [f32; PIECE_RECORD_FLOATS] {
        [
            self.size_px[0],
            self.size_px[1],
            self.light_dir[0],
            self.light_dir[1],
            self.glyph_rect_px[0],
            self.glyph_rect_px[1],
            self.glyph_rect_px[2],
            self.glyph_rect_px[3],
            atlas[0],
            atlas[1],
            atlas[2],
            atlas[3],
            self.tape_color[0],
            self.tape_color[1],
            self.tape_color[2],
            self.tape_color[3],
            self.letter_color[0],
            self.letter_color[1],
            self.letter_color[2],
            self.letter_color[3],
            self.bevel_px,
            self.dilate_px,
            self.sheen_amount,
            self.grain_amount,
            self.seed,
            0.0,
            0.0,
            0.0,
        ]
    }
}


/// A record's bytes, for a buffer write.
pub fn record_bytes(record: &[f32; PIECE_RECORD_FLOATS]) -> Vec<u8> {
    record.iter().flat_map(|f| f.to_ne_bytes()).collect()
}
