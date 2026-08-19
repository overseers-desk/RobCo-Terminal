//! The uniform payloads of the three procedural metal shaders: what a host
//! (a shell's furniture, the app's column) fills in to draw a metal, and
//! what the shader-oracle test crate's CPU reimplementations take as input.

pub struct MetalParams {
    pub grain_amount: f32,
    pub mottle_amount: f32,
    pub scratch_amount: f32,
}


pub struct ChassisMetalParams {
    pub field_scale: [f32; 2],
    pub field_offset: [f32; 2],
    pub light_dir: [f32; 2],
    pub chassis_color: [f32; 3],
    pub metal: MetalParams,
    pub vignette_strength: f32,
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

