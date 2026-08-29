//! The one join between the art and the settings file.
//!
//! `critters::ART` names the pieces; `[critters]` carries a key per piece so
//! any of them can be retired. Nothing in the type system holds those two
//! lists together, and the cost of them drifting apart is quiet: a piece
//! added to the art with no key would cross the glass with no way to switch
//! it off, and a key left behind by a piece that went away would be a
//! checkbox that did nothing.

use std::collections::BTreeSet;

/// The `[critters]` table of the settings dump, which is the document the
/// settings window opens on: keys as the window will see them, not as the
/// schema happens to spell them today.
fn table() -> String {
    let dump = config::dump::dump(Vec::new());
    let from = dump
        .find("[critters]")
        .expect("the dump carries [critters]");
    let rest = &dump[from + "[critters]".len()..];
    let to = rest.find("\n[").unwrap_or(rest.len());
    rest[..to].to_string()
}

fn keys() -> BTreeSet<String> {
    table()
        .lines()
        .filter_map(|line| line.split_once('=').map(|(key, _)| key.trim().to_string()))
        .collect()
}

#[test]
fn every_piece_of_art_has_a_key_that_retires_it() {
    let keys = keys();
    for art in &critters::ART {
        assert!(
            keys.contains(art.name),
            "the art carries {} and [critters] has no key for it",
            art.name
        );
    }
}

#[test]
fn every_key_that_is_not_a_knob_names_a_piece_of_art() {
    let knobs: BTreeSet<String> = ["enabled", "mean_minutes"]
        .into_iter()
        .map(String::from)
        .collect();
    let names: BTreeSet<String> = critters::ART.iter().map(|a| a.name.to_string()).collect();
    for key in keys().difference(&knobs) {
        assert!(
            names.contains(key),
            "[critters] carries {key} and no piece of art answers to it"
        );
    }
}

/// The shipped state is the whole cast, on. A default that quietly retired a
/// piece would be a piece nobody ever sees.
#[test]
fn the_shipped_cast_is_all_of_them() {
    let table = table();
    for art in &critters::ART {
        assert!(
            table.contains(&format!("{} = true", art.name)),
            "{} is not in the shipped cast",
            art.name
        );
    }
}
