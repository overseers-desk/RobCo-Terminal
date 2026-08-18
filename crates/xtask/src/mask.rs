//! Blackens the judged-out regions of a screenshot -- the CRT glass, the LED
//! panel rows, and the page-number window where a profile's metrics declare
//! one -- so a comparison scores only the pixels that are actually judged
//! on.
//!
//! This shells out to an embedded python3+PIL script rather than
//! reimplementing rounded-rectangle image compositing in Rust, which would
//! be a real chance to introduce a subtly different antialiasing or
//! rounding result that would silently shift every RMSE number downstream.
use anyhow::{bail, Context, Result};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const SHINE_BAND: u32 = 12;

/// The carve-out script, piped to python3 on stdin.
const MASK_PY: &str = r#"
import json, sys
from PIL import Image, ImageDraw

json_path, img_path, out_path, band = sys.argv[1:5]
band = int(band)

with open(json_path) as f:
    m = json.load(f)

im = Image.open(img_path).convert("RGB")
draw = ImageDraw.Draw(im)

carve = m["carveouts"]

g = carve["crt_glass"]
draw.rounded_rectangle(
    [g["x0"] - band, g["y0"] - band, g["x1"] + band, g["y1"] + band],
    radius=g["radius"] + band, fill=(0, 0, 0))

led = carve["led_panels"]
r = led["row1"]
for i in range(led["count"]):
    dy = round(i * led["pitch"])
    draw.rounded_rectangle(
        [r["x0"], r["y0"] + dy, r["x1"], r["y1"] + dy],
        radius=r["radius"], fill=(0, 0, 0))

# The page-number window is an LED panel too, where the metrics declare one.
pw = carve.get("page_window")
if pw:
    draw.rounded_rectangle(
        [pw["x0"], pw["y0"], pw["x1"], pw["y1"]],
        radius=pw["radius"], fill=(0, 0, 0))

im.save(out_path)
print(out_path)
"#;

pub fn run(metrics: &Path, image: &Path, out: &Path) -> Result<()> {
    let json = std::fs::canonicalize(metrics)
        .with_context(|| format!("mock metrics json not found: {}", metrics.display()))?;
    let img = std::fs::canonicalize(image)
        .with_context(|| format!("image not found: {}", image.display()))?;
    let out_abs = make_absolute(out);

    let mut child = Command::new("python3")
        .arg("-")
        .arg(&json)
        .arg(&img)
        .arg(&out_abs)
        .arg(SHINE_BAND.to_string())
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .context("spawning python3 (PIL required: pip install pillow)")?;

    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(MASK_PY.as_bytes())
        .context("writing mask script to python3 stdin")?;

    let status = child.wait().context("waiting on python3")?;
    if !status.success() {
        bail!("mask.py exited with {status}");
    }
    Ok(())
}

fn make_absolute(p: &Path) -> PathBuf {
    if p.is_absolute() {
        p.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(p)
    }
}
