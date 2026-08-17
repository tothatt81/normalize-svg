// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.
//
// The fonts under `assets/` are NOT covered by this notice — see NOTICE.md.

use std::sync::Arc;
use std::sync::Mutex;

use wasm_bindgen::prelude::*;

mod fontface;
use fontface::embedded_fonts;

/// Arimo (OFL 1.1, assets/ARIMO-LICENSE.md), for every family we hold no face for.
/// Metric-compatible with Arial and Liberation Sans, so a sticker naming either lays out at the
/// width it expected.
static TEXT_FONT: &[u8] = include_bytes!("../assets/Arimo.ttf");

/// Twemoji in COLR/CPAL **v0** (art CC-BY 4.0, assets/TWEMOJI-LICENSE.md): flat-filled layer
/// stacks rather than COLRv1 paint graphs, which is what usvg's COLR translation handles cleanly.
static EMOJI_FONT: &[u8] = include_bytes!("../assets/twemoji.ttf");

/// One face selection can choose: either bundled, or lifted out of the SVG's `@font-face` rules.
struct Face {
    id: usvg::fontdb::ID,
    /// Canonical (trimmed, unquoted, lowercased) family name.
    family: String,
}

/// Serves every family we hold no face for, and CSS generics.
const FALLBACK_FAMILY: &str = "arimo";
const EMOJI_FAMILY: &str = "twemoji";
/// The emoji family an SVG may name that no headless renderer can have, mapped onto ours;
/// `Segoe UI Emoji` is what Excalidraw declares. Only a family we want to supply the span's
/// *metrics* belongs here — any other emoji name still reaches the emoji font through
/// `select_fallback`, only the advances differ.
const EMOJI_ALIAS: &str = "segoe ui emoji";

/// What the SVG brought, and what we always have. `text` and `emoji` are always present, which
/// is what lets `select_font` answer an `ID` rather than an `Option`.
struct Registry {
    /// Faces the SVG carried in its own `@font-face` rules, in declaration order.
    faces: Vec<Face>,
    text: Face,
    emoji: Face,
}

impl Registry {
    /// Every face, in the order fallback should try them.
    fn all(&self) -> impl Iterator<Item = &Face> {
        self.faces.iter().chain([&self.emoji, &self.text])
    }
}

/// The font database usvg parses against, plus the metadata selection reads from it.
///
/// Built per call, because the `@font-face` rules an SVG carries are not constant and building
/// costs about what the normalization it serves does. So the module keeps no shared state, and a
/// test can hand in a database of its own. Database and registry travel together because a
/// `fontdb::ID` only means anything against the database it came from.
fn load_fonts(svg: &str) -> (Arc<usvg::fontdb::Database>, Registry) {
    let mut db = usvg::fontdb::Database::new();

    let add = |db: &mut usvg::fontdb::Database, bytes: Vec<u8>, family: &str| -> Option<Face> {
        let before = db.len();
        db.load_font_data(bytes);
        // A collection could add several; the first is the one the rule named.
        db.faces().nth(before).map(|face| Face {
            id: face.id,
            family: family.to_string(),
        })
    };

    let text = add(&mut db, TEXT_FONT.to_vec(), FALLBACK_FAMILY).expect("bundled text font");
    let emoji = add(&mut db, EMOJI_FONT.to_vec(), EMOJI_FAMILY).expect("bundled emoji font");

    let mut faces = Vec::new();
    for (family, bytes) in embedded_fonts(svg) {
        if let Some(face) = add(&mut db, bytes, &canonical(&family)) {
            faces.push(face);
        }
    }

    (Arc::new(db), Registry { faces, text, emoji })
}

fn canonical(name: &str) -> String {
    name.trim()
        .trim_matches(['"', '\''])
        .trim()
        .to_ascii_lowercase()
}

fn build_options(db: Arc<usvg::fontdb::Database>, registry: Registry) -> usvg::Options<'static> {
    let registry = Arc::new(registry);
    let for_span = registry.clone();
    let for_fallback = registry;

    // The span's resolved family order, handed from one resolver to the other: usvg gives
    // `select_fallback` a codepoint and nothing else, so it can only prefer the order the font
    // string asked for by being told, and `select_font` is where that order is known.
    //
    // A `Mutex` rather than a `RefCell` because `FontResolver`'s closures are `Send + Sync`.
    // Correct as long as usvg finishes one span before starting the next — it shapes them
    // sequentially, and `each_span_gets_its_own_chain` pins that down.
    let chain = Arc::new(Mutex::new(Vec::new()));
    let for_span_chain = chain.clone();
    let for_fallback_chain = chain;

    let lock = |m: &Mutex<Vec<usvg::fontdb::ID>>| match m.lock() {
        Ok(guard) => guard.clone(),
        // A poisoned lock means a previous span panicked mid-resolve; the order is only a
        // preference, so carry on without it rather than taking the whole render down.
        Err(poisoned) => poisoned.into_inner().clone(),
    };

    usvg::Options {
        fontdb: db,
        font_resolver: usvg::FontResolver {
            // Only `select_fallback` can answer `None`, once every face has been tried;
            // usvg then leaves the cluster to the span font's own .notdef, a visible box.
            select_font: Box::new(move |font, _| {
                let chain = span_chain(&for_span, font.families());
                let head = select_font(&for_span, &chain);
                match for_span_chain.lock() {
                    Ok(mut slot) => *slot = chain,
                    Err(poisoned) => *poisoned.into_inner() = chain,
                }
                Some(head)
            }),
            select_fallback: Box::new(move |codepoint, exclude, db| {
                let chain = lock(&for_fallback_chain);
                select_fallback(&for_fallback, &chain, db, codepoint, exclude)
            }),
        },
        ..usvg::Options::default()
    }
}

/// The span's font: the first family in its stack we hold a face for, else the text font.
///
/// Supplies the span's **metrics**, not only its glyphs, so it must be a real face of the
/// intended family. Weight and style go unread — one face per family, no synthesised bold.
/// Never `None`: the text font is always registered, and `None` would make usvg drop the whole
/// `<text>` node rather than fall back.
fn select_font(registry: &Registry, chain: &[usvg::fontdb::ID]) -> usvg::fontdb::ID {
    chain.first().copied().unwrap_or(registry.text.id)
}

/// The faces a span's font string names, resolved in the order it names them.
///
/// The head is the span font — it sets the metrics for the whole run — and the tail is the order
/// `select_fallback` should prefer. Without it, a span asking for `"sub, b"` and one asking for
/// `"sub, c"` resolve the same fallback, because `Registry::all` only knows registration order.
fn span_chain(registry: &Registry, families: &[usvg::FontFamily]) -> Vec<usvg::fontdb::ID> {
    let mut chain = Vec::new();
    let mut push = |id| {
        if !chain.contains(&id) {
            chain.push(id);
        }
    };

    for family in families {
        // A CSS generic names no face of ours. Breaking also discards families *after* it —
        // Excalidraw declares `Excalifont, Xiaolai, sans-serif, Segoe UI Emoji`, and its trailing
        // emoji family should lose the span, since the span font sets the metrics for the whole
        // run and the text font is the Arial-matched one. Emoji still reach `select_fallback`.
        let name = match family {
            usvg::FontFamily::Named(name) => canonical(name),
            _ => break,
        };
        // An embedded face wins even over the emoji alias: the SVG shipped its own.
        if let Some(face) = registry.faces.iter().find(|face| face.family == name) {
            push(face.id);
        } else if name == EMOJI_ALIAS {
            push(registry.emoji.id);
        }
    }

    chain
}

/// The next untried face that can actually render `codepoint`, for a cluster the span's own font
/// could not shape.
///
/// Filters `Registry::all`'s order by what each face's cmap covers, and declines the emoji face
/// for a bare keycap base (see [`is_keycap_base`]).
///
/// usvg asks once per span rather than once per cluster, re-shapes the whole text with whatever
/// we hand back, and merges the two shapings on shared cluster boundaries. That merge is the
/// patch we pin usvg for — see the note in `Cargo.toml`.
fn select_fallback(
    registry: &Registry,
    chain: &[usvg::fontdb::ID],
    db: &usvg::fontdb::Database,
    codepoint: char,
    exclude: &[usvg::fontdb::ID],
) -> Option<usvg::fontdb::ID> {
    chain
        .iter()
        .copied()
        // Then everything else: faces the span did not name, and the two bundled ones. The
        // chain's own entries reappear here and are simply re-tested; `find` takes the first.
        .chain(registry.all().map(|face| face.id))
        .filter(|id| !exclude.contains(id))
        .filter(|id| !(*id == registry.emoji.id && is_keycap_base(codepoint)))
        .find(|id| covers(db, *id, codepoint))
}

/// The keycap bases: `#`, `*` and `0`-`9`.
///
/// The emoji font maps all twelve (the `ccmp` ligature for `<base> U+FE0F U+20E3` needs its
/// inputs reachable through the cmap) but their glyphs are blank, so picking it for a *bare*
/// base renders nothing at all and the text font is never asked. U+FE0F and U+20E3 are
/// deliberately absent: that is how a real keycap sequence reaches the face once the base has
/// been served.
fn is_keycap_base(codepoint: char) -> bool {
    matches!(codepoint, '#' | '*' | '0'..='9')
}

/// Whether a face's cmap maps `codepoint`.
///
/// usvg's contract for `select_fallback` is that the face we answer with supports the character.
/// Answering *without* checking is what let the emoji font swallow bare digits.
fn covers(db: &usvg::fontdb::Database, id: usvg::fontdb::ID, codepoint: char) -> bool {
    db.with_face_data(id, |data, index| {
        ttf_parser::Face::parse(data, index)
            .map(|face| face.glyph_index(codepoint).is_some())
            .unwrap_or(false)
    })
    .unwrap_or(false)
}

/// usvg indexes a 13-entry table by precision (`POW_VEC[precision as usize]` in its writer), so
/// anything above 12 panics — which in wasm is an uncatchable trap, not an error. Validating here
/// keeps a caller's typo from taking the module down.
const MAX_PRECISION: u8 = 12;

fn precision(value: Option<u8>, name: &str, default: u8) -> Result<u8, JsError> {
    match value {
        None => Ok(default),
        Some(v) if v <= MAX_PRECISION => Ok(v),
        Some(v) => Err(JsError::new(&format!(
            "{name} must be between 0 and {MAX_PRECISION}, got {v}"
        ))),
    }
}

/// Normalizes `svg` into the subset a partial SVG parser handles well: `<use>`, nested `<svg>`,
/// CSS, shape elements **and `<text>`** all become plain `<path>`.
///
/// Text is outlined here rather than left to the caller precisely because the fonts are embedded:
/// resolving glyphs at this layer is what makes the output identical on every OS.
///
/// Both precisions default to usvg's own (8) and accept 0-12. Lowering them shrinks the output
/// substantially, which matters when the consumer copies coordinates verbatim: a PDF exporter
/// writes them straight into the content stream, where full-precision floats left embedded-SVG-heavy
/// PDFs 85%-digits by weight. 3 is a good value there — far below visibility at any plausible
/// scale — but it is the caller's call to make, not this crate's.
#[wasm_bindgen]
pub fn normalize(
    svg: &str,
    coordinates_precision: Option<u8>,
    transforms_precision: Option<u8>,
) -> Result<String, JsError> {
    let defaults = usvg::WriteOptions::default();
    let write_options = usvg::WriteOptions {
        coordinates_precision: precision(
            coordinates_precision,
            "coordinates_precision",
            defaults.coordinates_precision,
        )?,
        transforms_precision: precision(
            transforms_precision,
            "transforms_precision",
            defaults.transforms_precision,
        )?,
        ..defaults
    };

    let (db, registry) = load_fonts(svg);
    let options = build_options(db, registry);

    let tree = usvg::Tree::from_str(svg, &options)
        .map_err(|error| JsError::new(&format!("Failed to parse SVG: {error}")))?;

    Ok(tree.to_string(&write_options))
}

#[cfg(test)]
mod tests;
