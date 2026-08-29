//! The uniform payloads of the three procedural metal shaders: what a host
//! (a shell's furniture, the app's column) fills in to draw a metal, and
//! what the shader-oracle test crate's CPU reimplementations take as input.

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

