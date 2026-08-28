//! Colour resolution: rio-vt hands out `AnsiColor`, the renderer wants four
//! floats.
//!
//! The CRT pipeline that will sit downstream of this renderer is monochrome by
//! design (the whole grid is painted in one phosphor colour and the shader
//! chain does the rest), so the default scheme here is exactly that:
//! `Scheme::monochrome`. The full palette exists anyway, because a terminal
//! that silently discards SGR colours fails half the conformance suite and
//! every `ls --color`, and because the phosphor decision belongs to the CRT
//! shader passes rather than to the glyph path.

use rio_vt::config::colors::{AnsiColor, ColorRgb, NamedColor};

/// Straight, non-premultiplied RGBA. Premultiplication happens in the shader,
/// once, so nothing upstream has to remember which convention it is in.
pub type Rgba = [f32; 4];

pub const fn rgb(r: u8, g: u8, b: u8) -> Rgba {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

pub const TRANSPARENT: Rgba = [0.0, 0.0, 0.0, 0.0];

/// The colours a grid is drawn with.
#[derive(Clone, Debug)]
pub struct Scheme {
    pub foreground: Rgba,
    pub background: Rgba,
    pub cursor: Rgba,
    /// Indices 0..16 are the ANSI set, 16..232 the 6x6x6 cube, 232..256 the
    /// greys. Filled by `Scheme::xterm_palette` unless a caller overrides it.
    pub palette: [Rgba; 256],
    /// Dim (SGR 2) multiplier: matches xterm's 2/3 convention.
    pub dim_factor: f32,
}

impl Default for Scheme {
    fn default() -> Self {
        // Amber, the "Default Amber" phosphor profile every RMSE comparison
        // in this repo is taken at.
        Self::monochrome(rgb(255, 176, 0), [0.0, 0.0, 0.0, 1.0])
    }
}

impl Scheme {
    /// One phosphor colour for every glyph, one for every background. SGR
    /// colours still change *which* of the two a cell gets (through INVERSE),
    /// they just cannot introduce a third colour.
    pub fn monochrome(foreground: Rgba, background: Rgba) -> Self {
        Self {
            foreground,
            background,
            cursor: foreground,
            palette: [foreground; 256],
            dim_factor: 2.0 / 3.0,
        }
    }

    /// The ordinary 256-colour terminal palette, for when the phosphor is not
    /// being enforced (screenshots, conformance runs, `ls --color`).
    pub fn full_color(foreground: Rgba, background: Rgba) -> Self {
        Self {
            foreground,
            background,
            cursor: foreground,
            palette: xterm_palette(),
            dim_factor: 2.0 / 3.0,
        }
    }

    pub fn resolve(&self, color: AnsiColor) -> Rgba {
        match color {
            AnsiColor::Named(named) => self.named(named),
            AnsiColor::Spec(ColorRgb { r, g, b }) => rgb(r, g, b),
            AnsiColor::Indexed(i) => self.palette[i as usize],
        }
    }

    pub fn named(&self, named: NamedColor) -> Rgba {
        match named {
            NamedColor::Foreground => self.foreground,
            NamedColor::Background => self.background,
            NamedColor::Cursor => self.cursor,
            // The dim and bright families are the base sixteen with a
            // multiplier or an offset; `NamedColor as usize` puts the base set
            // at 0..16 and everything else above 256.
            other => {
                let i = other as usize;
                if i < 16 {
                    self.palette[i]
                } else if (NamedColor::DimBlack as usize..=NamedColor::DimWhite as usize)
                    .contains(&i)
                {
                    dim(
                        self.palette[i - NamedColor::DimBlack as usize],
                        self.dim_factor,
                    )
                } else if i == NamedColor::DimForeground as usize {
                    dim(self.foreground, self.dim_factor)
                } else if i == NamedColor::LightForeground as usize {
                    self.foreground
                } else {
                    self.foreground
                }
            }
        }
    }
}

pub fn dim(c: Rgba, factor: f32) -> Rgba {
    [c[0] * factor, c[1] * factor, c[2] * factor, c[3]]
}

/// The xterm 256-colour table, generated rather than transcribed: the cube and
/// the grey ramp are formulas, and a transcription of 240 hex triples is 240
/// chances to fat-finger one.
pub fn xterm_palette() -> [Rgba; 256] {
    const BASE: [(u8, u8, u8); 16] = [
        (0, 0, 0),
        (205, 0, 0),
        (0, 205, 0),
        (205, 205, 0),
        (0, 0, 238),
        (205, 0, 205),
        (0, 205, 205),
        (229, 229, 229),
        (127, 127, 127),
        (255, 0, 0),
        (0, 255, 0),
        (255, 255, 0),
        (92, 92, 255),
        (255, 0, 255),
        (0, 255, 255),
        (255, 255, 255),
    ];
    const CUBE: [u8; 6] = [0, 95, 135, 175, 215, 255];

    let mut out = [[0.0; 4]; 256];
    for (i, (r, g, b)) in BASE.iter().enumerate() {
        out[i] = rgb(*r, *g, *b);
    }
    for i in 0..216 {
        let (r, g, b) = (CUBE[i / 36], CUBE[(i / 6) % 6], CUBE[i % 6]);
        out[16 + i] = rgb(r, g, b);
    }
    for i in 0..24 {
        let v = 8 + i as u8 * 10;
        out[232 + i] = rgb(v, v, v);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_landmarks_match_xterm() {
        let p = xterm_palette();
        assert_eq!(p[1], rgb(205, 0, 0));
        assert_eq!(p[16], rgb(0, 0, 0));
        assert_eq!(p[21], rgb(0, 0, 255));
        assert_eq!(p[231], rgb(255, 255, 255));
        assert_eq!(p[232], rgb(8, 8, 8));
        assert_eq!(p[255], rgb(238, 238, 238));
    }

    /// The scheme the appliance runs keeps every palette entry's own colour,
    /// so a program that paints a background gets a cell lit as far as that
    /// colour was bright and no further. Collapsing the palette to the
    /// foreground instead lights every background fully, which fills the
    /// glass and hides the text drawn on it: `whiptail`'s backdrop is this
    /// blue, and it is meant to arrive almost dark.
    ///
    /// The weighing is `rgb2grey` in the chain's last pass, so this asserts
    /// the colours reach it, not what it makes of them.
    #[test]
    fn the_appliance_scheme_carries_a_background_colour_at_its_own_brightness() {
        let white = rgb(255, 255, 255);
        let scheme = Scheme::full_color(white, [0.0, 0.0, 0.0, 1.0]);
        let blue = scheme.resolve(AnsiColor::Named(NamedColor::Blue));
        assert_eq!(blue, rgb(0, 0, 238));
        assert_ne!(blue, scheme.foreground);
    }

    #[test]
    fn monochrome_scheme_cannot_produce_a_third_colour() {
        let fg = rgb(255, 176, 0);
        let scheme = Scheme::monochrome(fg, [0.0, 0.0, 0.0, 1.0]);
        for i in 0..=255u8 {
            assert_eq!(scheme.resolve(AnsiColor::Indexed(i)), fg);
        }
        assert_eq!(scheme.resolve(AnsiColor::Named(NamedColor::Red)), fg);
        // A truecolour SGR is the one escape that can still name its own
        // colour; the phosphor is enforced by the pass chain, not here.
        assert_eq!(
            scheme.resolve(AnsiColor::Spec(ColorRgb { r: 1, g: 2, b: 3 })),
            rgb(1, 2, 3)
        );
    }
}
