//! Fan-out measurement for the setting-duplication report (GitHub issue #4
//! on this repository): how many non-test homes a setting has.
//!
//! `fanout <term>` prints, per file under `crates/*/src` and `crates/*/shaders`,
//! the lines mentioning the term after stripping inline `#[cfg(test)]` modules,
//! then a total. `fanout --loc` prints the same non-test line count for the
//! whole workspace via `cloc`. Counts here are trend evidence, read beside a
//! count of some setting the change under measure never touched, so a rise
//! from mere editing churn shows in both. The structural verdict is taken
//! separately, by adding one throwaway setting end to end (or deleting a
//! real one) and counting the files a maintainer had to edit.

use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

/// Return `src` with every inline `#[cfg(test)] mod ... { }` block removed,
/// by brace counting from the attribute line.
fn strip_inline_tests(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut depth: i32 = 0;
    let mut in_test = false;
    let mut pending_attr = false;
    for line in src.lines() {
        if !in_test && line.trim_start().starts_with("#[cfg(test)]") {
            pending_attr = true;
            continue;
        }
        if pending_attr {
            pending_attr = false;
            in_test = true;
            depth = 0;
        }
        if in_test {
            depth += line.matches('{').count() as i32;
            depth -= line.matches('}').count() as i32;
            if depth <= 0 && line.contains('}') {
                in_test = false;
            }
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub fn fanout(term: &str) -> Result<()> {
    let mut total_lines = 0usize;
    let mut total_files = 0usize;
    let mut walk = vec![Path::new("crates").to_path_buf()];
    let mut hits = Vec::new();
    while let Some(dir) = walk.pop() {
        for entry in std::fs::read_dir(&dir).with_context(|| dir.display().to_string())? {
            let path = entry?.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if name != "target" && name != "tests" {
                    walk.push(path);
                }
                continue;
            }
            let rust = name.ends_with(".rs");
            if !rust && !name.ends_with(".slang") && !name.ends_with(".slangp") {
                continue;
            }
            let src = std::fs::read_to_string(&path).with_context(|| path.display().to_string())?;
            let body = if rust { strip_inline_tests(&src) } else { src };
            let n = body
                .lines()
                .filter(|l| l.to_lowercase().contains(&term.to_lowercase()))
                .count();
            if n > 0 {
                hits.push((path.display().to_string(), n));
                total_lines += n;
                total_files += 1;
            }
        }
    }
    hits.sort();
    for (path, n) in &hits {
        println!("{n:5}  {path}");
    }
    println!("{term}: {total_lines} non-test lines in {total_files} files");
    Ok(())
}

pub fn loc() -> Result<()> {
    let out = Command::new("cloc")
        .args(["--quiet", "--include-lang=Rust", "--not-match-d=target", "crates"])
        .output()
        .context("cloc")?;
    print!("{}", String::from_utf8_lossy(&out.stdout));
    println!("(cloc counts include inline #[cfg(test)] modules; compare like with like across runs)");
    Ok(())
}
