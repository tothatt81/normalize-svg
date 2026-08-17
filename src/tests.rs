// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.
//
// The fonts under `assets/` are NOT covered by this notice — see NOTICE.md.

//! Tests for the font registry and the two usvg resolvers.

use super::*;

/// Adobe NotDef (OFL, see assets/AND-LICENSE.md) draws every glyph as a rectangle, which makes
/// it the ideal oracle for "did the embedded face actually win?": zero curve commands means it
/// did, any curves mean we fell back to Arimo. Shared with the `@font-face` tests.
pub(crate) static NOTDEF_FONT: &[u8] = include_bytes!("../assets/AND-Regular.otf");
/// The same font compressed with `fontTools.ttLib` (`f.flavor = "woff2"`), which is the format
/// Excalidraw embeds and the one `fontdb` cannot read unaided.
pub(crate) static NOTDEF_WOFF2: &[u8] = include_bytes!("../tests/fixtures/AND-Regular.woff2");
/// Arimo subset to `A` alone (`fontTools.subset`, `unicodes=[0x41]`). A span font that covers
/// *some* of its text is what makes `select_fallback` run at all, so this is the fixture the
/// digit-loss regression needs: every other character in the span has to be resolved elsewhere.
static A_ONLY_FONT: &[u8] = include_bytes!("../tests/fixtures/A-only.ttf");

fn run_normalize(svg: &str) -> String {
    normalize(svg, None, None)
        .map_err(|_| "normalize failed")
        .unwrap()
}

#[test]
fn shapes_flatten_to_paths() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20">
      <defs><rect id="r" width="10" height="10" fill="red"/></defs><use href="#r"/>
    </svg>"##;
    let output = run_normalize(svg);
    assert!(output.contains("<path"));
    assert!(!output.contains("<use"));
    assert!(!output.contains("<rect"));
}

/// Neither family is one we ship, so the span falls back to the inlined text font — which
/// covers Latin. That geometry comes out is the point: usvg drops a `<text>` node whose
/// span resolves no font at all, so `select_font` must never answer `None`.
#[test]
fn text_becomes_paths_even_with_no_font_registered() {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="50">
      <text x="10" y="30" font-family="Fancy, Emoji" font-size="20">Hi!</text>
    </svg>"#;
    let output = run_normalize(svg);
    assert!(
        !output.contains("<text"),
        "text is converted, not preserved"
    );
    assert!(output.contains("<path"));
    assert!(!output.contains("Hi!"), "the characters are geometry now");
}

#[test]
fn emoji_text_becomes_paths_too() {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"50\">\
      <text x=\"10\" y=\"30\" font-size=\"20\">\u{1F600}\u{1F389}</text></svg>";
    let output = run_normalize(svg);
    assert!(!output.contains("<text"));
    assert!(output.contains("<path"));
}

// The invalid-SVG error path is exercised from JS (usvg.test.ts): JsError cannot
// be constructed outside a wasm runtime, so a native test would panic in the shim.

/// Compound emoji survive font fallback.
///
/// The whole reason `Cargo.toml` pins usvg to a git rev instead of a crates.io release. A
/// multi-codepoint cluster ligates to one glyph in the emoji font while the text font
/// leaves one .notdef per codepoint; stock usvg merges the two shapings by glyph index and
/// abandons the merge when the counts differ, so every emoji in the span comes out as a
/// box. Should we ever drift back onto an unpatched usvg, this is what catches it.
#[test]
fn compound_emoji_survive_fallback_in_a_mixed_span() {
    // Four ways to spell a cluster longer than one codepoint, none of them shared: a
    // variation selector, a skin-tone modifier, a regional-indicator pair, and a ZWJ join.
    for (label, text) in [
        ("VS16", "Hi \u{2764}\u{FE0F}"),
        ("skin tone", "Hi \u{1F44D}\u{1F3FD}"),
        ("flag", "Hi \u{1F1ED}\u{1F1FA}"),
        ("ZWJ", "Hi \u{1F468}\u{200D}\u{1F467}"),
        (
            "one cluster poisoning another",
            "Hi \u{1F389}\u{2764}\u{FE0F}",
        ),
    ] {
        let svg = format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 400 80\">\
             <text x=\"10\" y=\"42\" font-size=\"36\">{text}</text></svg>"
        );
        let output = run_normalize(&svg);
        let fills: std::collections::BTreeSet<&str> = output
            .match_indices("fill=\"#")
            .map(|(i, _)| &output[i + 7..i + 13])
            .collect();

        // COLR layers paint in the palette's colours; a tofu box would only ever be the
        // text fill. Any non-black fill means the emoji font's glyphs actually landed.
        assert!(
            fills.iter().any(|fill| *fill != "000000"),
            "{label}: {text:?} produced only {fills:?} - the emoji fell back to tofu"
        );
    }
}

fn face(id: usvg::fontdb::ID, family: &str) -> Face {
    Face {
        id,
        family: family.to_string(),
    }
}

/// Shaped like the one `load_fonts` builds: no embedded faces, plus the two bundled ones —
/// and loaded from the *real* font bytes, because `select_fallback` now reads their cmaps.
/// `ids.0` is the text font and `ids.1` the emoji font.
fn registry() -> (Registry, Vec<usvg::fontdb::ID>, usvg::fontdb::Database) {
    let mut db = usvg::fontdb::Database::new();
    db.load_font_data(TEXT_FONT.to_vec());
    db.load_font_data(EMOJI_FONT.to_vec());
    let ids: Vec<_> = db.faces().map(|face| face.id).collect();
    let registry = Registry {
        faces: Vec::new(),
        text: face(ids[0], FALLBACK_FAMILY),
        emoji: face(ids[1], EMOJI_FAMILY),
    };
    (registry, ids, db)
}

/// A `<text>` in `family`, wrapped in a stylesheet declaring `family` from `font` bytes.
fn svg_with_embedded_font(family: &str, mime: &str, font: &[u8], text: &str) -> String {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64;
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"200\" height=\"50\"><defs><style>\
         @font-face {{ font-family: {family}; src: url(data:font/{mime};base64,{}); }}\
         </style></defs>\
         <text x=\"10\" y=\"30\" font-family=\"{family}\" font-size=\"20\">{text}</text></svg>",
        BASE64.encode(font)
    )
}

/// Curve commands in the output. Adobe NotDef draws every glyph as a rectangle, so zero
/// curves means the embedded face won and any curves mean we fell back to Arimo.
fn curve_count(svg: &str) -> usize {
    let output = run_normalize(svg);
    output
        .match_indices(" d=\"")
        .map(|(i, _)| {
            let rest = &output[i + 4..];
            let end = rest.find('"').unwrap_or(0);
            rest[..end].matches(['Q', 'C']).count()
        })
        .sum()
}

#[test]
fn fallback_moves_on_to_the_face_the_span_has_not_tried() {
    let (registry, ids, db) = registry();
    // Asked on behalf of the text font, for a character it lacks.
    let picked = select_fallback(&registry, &[], &db, '\u{1F600}', &[ids[0]]);
    assert_eq!(picked, Some(ids[1]));
    // And the other way round: the enumeration reaches every face either way.
    let picked = select_fallback(&registry, &[], &db, 'A', &[ids[1]]);
    assert_eq!(picked, Some(ids[0]));
}

#[test]
fn fallback_skips_a_face_whose_cmap_lacks_the_character() {
    let (registry, ids, db) = registry();
    // Nothing is excluded, so the emoji font is offered first by position — but it has no
    // Latin 'A', and usvg's contract is that we answer with a face that supports the char.
    assert_eq!(select_fallback(&registry, &[], &db, 'A', &[]), Some(ids[0]));
}

/// The digit-loss regression, at the resolver.
///
/// The emoji font maps `#`, `*` and `0`-`9` so its keycap `ccmp` inputs are reachable, but
/// their glyphs are blank. Offering it for a bare digit satisfies usvg's merge with nothing
/// and the text font is never asked — every digit in the span silently disappears.
#[test]
fn emoji_face_is_never_offered_for_a_bare_keycap_base() {
    let (registry, ids, db) = registry();
    for c in ['3', '0', '9', '#', '*'] {
        assert_eq!(
            select_fallback(&registry, &[], &db, c, &[]),
            Some(ids[0]),
            "{c:?} must come from the text font, not the blank emoji glyph"
        );
    }
    // …and with the text font already tried there is nothing left to offer.
    assert_eq!(select_fallback(&registry, &[], &db, '3', &[ids[0]]), None);
}

/// The chain is what the font string asked for, in that order — including the emoji alias,
/// and stopping at a CSS generic the way `select_font` always has.
#[test]
fn the_span_chain_follows_the_font_string() {
    let (mut registry, ids, db) = registry();
    let _ = &db;
    let mut more = usvg::fontdb::Database::new();
    more.load_font_data(TEXT_FONT.to_vec());
    more.load_font_data(TEXT_FONT.to_vec());
    let extra: Vec<_> = more.faces().map(|face| face.id).collect();
    registry.faces = vec![face(extra[0], "b"), face(extra[1], "c")];

    let named = |names: &[&str]| -> Vec<usvg::FontFamily> {
        names
            .iter()
            .map(|n| usvg::FontFamily::Named((*n).to_string()))
            .collect()
    };

    assert_eq!(span_chain(&registry, &named(&["b", "c"])), vec![extra[0], extra[1]]);
    // …and the other order, which is the whole point.
    assert_eq!(span_chain(&registry, &named(&["c", "b"])), vec![extra[1], extra[0]]);
    // Unknown families contribute nothing; the emoji alias resolves to the bundled face.
    assert_eq!(
        span_chain(&registry, &named(&["nope", "segoe ui emoji"])),
        vec![ids[1]]
    );
    // A generic truncates, so a trailing emoji family never wins the span.
    let mut with_generic = named(&["nope"]);
    with_generic.push(usvg::FontFamily::SansSerif);
    with_generic.push(usvg::FontFamily::Named("segoe ui emoji".to_string()));
    assert_eq!(span_chain(&registry, &with_generic), Vec::new());
}

/// Fallback prefers the order the span asked for, not the order the stylesheet declared.
///
/// Both faces cover the character, and both are registered in the same order either way — so
/// before the chain, `"sub, b"` and `"sub, c"` resolved to the *same* face.
#[test]
fn fallback_prefers_the_order_the_span_asked_for() {
    let (mut registry, _ids, mut db) = registry();
    db.load_font_data(TEXT_FONT.to_vec());
    db.load_font_data(TEXT_FONT.to_vec());
    let all: Vec<_> = db.faces().map(|face| face.id).collect();
    let (b, c) = (all[2], all[3]);
    registry.faces = vec![face(b, "b"), face(c, "c")];

    assert_eq!(select_fallback(&registry, &[c], &db, 'A', &[]), Some(c));
    assert_eq!(select_fallback(&registry, &[b], &db, 'A', &[]), Some(b));
    // With no chain it is registration order, as before.
    assert_eq!(select_fallback(&registry, &[], &db, 'A', &[]), Some(b));
}

/// Two spans in one SVG each get their own chain.
///
/// The chain is handed from `select_font` to `select_fallback` through shared state, which is
/// only sound because usvg resolves one span before starting the next. If that ever stopped
/// holding, the second span would inherit the first's order and this would catch it: `b` draws
/// notdef rectangles (no curves) and `c` draws real Arimo digits (curves), so a leaked chain
/// makes the two spans render identically instead of differently.
#[test]
fn each_span_gets_its_own_chain() {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD as BASE64;

    let faces = format!(
        "@font-face {{ font-family: sub; src: url(data:font/ttf;base64,{}); }}\
         @font-face {{ font-family: b; src: url(data:font/otf;base64,{}); }}\
         @font-face {{ font-family: c; src: url(data:font/ttf;base64,{}); }}",
        BASE64.encode(A_ONLY_FONT),
        BASE64.encode(NOTDEF_FONT),
        BASE64.encode(TEXT_FONT),
    );
    let svg = |texts: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"400\" height=\"80\">\
             <defs><style>{faces}</style></defs>{texts}</svg>"
        )
    };
    let via_b = "<text x=\"10\" y=\"30\" font-family=\"sub, b\" font-size=\"20\">A3</text>";
    let via_c = "<text x=\"10\" y=\"60\" font-family=\"sub, c\" font-size=\"20\">A3</text>";

    let only_b = curve_count(&svg(via_b));
    let only_c = curve_count(&svg(via_c));
    assert!(
        only_c > only_b,
        "the two chains must be distinguishable: b={only_b} c={only_c}"
    );

    // Both spans together: each keeps its own answer, so the curves simply add up. A leaked
    // chain would give 2*only_b or 2*only_c instead.
    assert_eq!(curve_count(&svg(&format!("{via_b}{via_c}"))), only_b + only_c);
    // …and in the other document order, in case the leak ran the other way.
    assert_eq!(curve_count(&svg(&format!("{via_c}{via_b}"))), only_b + only_c);
}

/// The digit-loss regression, end to end.
///
/// A span whose own face covers `A` but no digits: before the coverage rule, the fallback
/// enumeration offered the emoji font first, its blank `3` satisfied usvg's merge, and the
/// digits came out as nothing at all — `"A123"` drew exactly as much geometry as `"A"`.
#[test]
fn digits_survive_a_span_font_that_lacks_them() {
    let curves_of = |text: &str| {
        curve_count(&run_normalize(&svg_with_embedded_font(
            "sub", "ttf", A_ONLY_FONT, text,
        )))
    };

    let a = curves_of("A");
    assert!(a > 0, "the embedded face should draw its own 'A'");

    for text in ["A3", "A123", "A3 \u{1F389}"] {
        assert!(
            curves_of(text) > a,
            "{text:?} drew {} curves, same as \"A\" alone ({a}) — the digits were swallowed",
            curves_of(text)
        );
    }
}

/// The other half of the same rule: a real keycap sequence still has to reach the emoji font.
#[test]
fn the_keycap_mark_still_reaches_the_emoji_face() {
    let (registry, ids, db) = registry();
    assert_eq!(select_fallback(&registry, &[], &db, '\u{20E3}', &[]), Some(ids[1]));
    assert_eq!(select_fallback(&registry, &[], &db, '\u{FE0F}', &[]), Some(ids[1]));
}

#[test]
fn fallback_yields_nothing_once_every_face_is_excluded() {
    let (registry, _ids, db) = registry();
    let all: Vec<_> = db.faces().map(|face| face.id).collect();
    // Passed through to usvg, which then leaves the cluster to the span font's own
    // .notdef glyph — a visible box rather than a deleted character.
    assert_eq!(select_fallback(&registry, &[], &db, 'Ж', &all), None);
}

#[test]
fn fallback_tries_the_svgs_own_faces_before_the_bundled_ones() {
    let mut db = usvg::fontdb::Database::new();
    db.load_font_data(TEXT_FONT.to_vec());
    db.load_font_data(EMOJI_FONT.to_vec());
    db.load_font_data(TEXT_FONT.to_vec()); // stands in for the SVG's own face
    let ids: Vec<_> = db.faces().map(|face| face.id).collect();
    let registry = Registry {
        faces: vec![face(ids[2], "excalifont")],
        text: face(ids[0], FALLBACK_FAMILY),
        emoji: face(ids[1], EMOJI_FAMILY),
    };
    assert_eq!(select_fallback(&registry, &[], &db, 'A', &[]), Some(ids[2]));
}

/// The embedded font is Adobe NotDef, so "Hello" is five boxes if the rule was honoured
/// and real Arimo letters if it was not.
#[test]
fn an_embedded_otf_face_serves_the_family_it_declares() {
    let svg = svg_with_embedded_font("EmbeddedFace", "otf", NOTDEF_FONT, "Hello");
    assert_eq!(curve_count(&svg), 0, "should be NotDef boxes, not Arimo");
}

#[test]
fn an_embedded_woff2_face_is_decompressed_and_used() {
    let svg = svg_with_embedded_font("EmbeddedFace", "woff2", NOTDEF_WOFF2, "Hello");
    assert_eq!(curve_count(&svg), 0, "should be NotDef boxes, not Arimo");
}

#[test]
fn an_unreadable_font_source_falls_back_instead_of_failing() {
    let svg = svg_with_embedded_font("EmbeddedFace", "woff2", b"not a font at all", "Hello");
    // Rendered, in Arimo — a broken `@font-face` must never fail the normalization.
    assert!(curve_count(&svg) > 0, "should have fallen back to Arimo");
}

#[test]
fn a_family_the_svg_did_not_embed_still_reaches_the_text_font() {
    let svg = svg_with_embedded_font("EmbeddedFace", "otf", NOTDEF_FONT, "Hello")
        .replace("font-family=\"EmbeddedFace\"", "font-family=\"Nothing We Have\"");
    assert!(curve_count(&svg) > 0, "should be Arimo letters");
}

#[test]
fn an_emoji_family_we_cannot_ship_still_paints_emoji() {
    let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 200 60\">\
               <text x=\"10\" y=\"40\" font-size=\"32\" font-family=\"Segoe UI Emoji\">\
               \u{1F389}</text></svg>";
    let output = run_normalize(svg);
    assert!(
        output.contains("fill=\"#") && !output.contains("fill=\"#000000\""),
        "the emoji alias should reach the bundled emoji font"
    );
}


/// Fractional coordinates and a fractional transform in one document, so both knobs have
/// something to round.
const FRACTIONAL: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"40\" height=\"40\">\
                          <g transform=\"rotate(13)\"><circle cx=\"5\" cy=\"5\" r=\"3\"/></g></svg>";

fn with_precision(coordinates: Option<u8>, transforms: Option<u8>) -> String {
    normalize(FRACTIONAL, coordinates, transforms)
        .map_err(|_| "normalize failed")
        .unwrap()
}

#[test]
fn precision_defaults_to_usvgs_own() {
    let defaults = usvg::WriteOptions::default();
    assert_eq!(
        with_precision(None, None),
        with_precision(
            Some(defaults.coordinates_precision),
            Some(defaults.transforms_precision)
        ),
        "omitting both should be identical to passing usvg's defaults, not a value of our own"
    );
}

#[test]
fn lower_precision_shortens_the_output() {
    let coarse = with_precision(Some(1), Some(1));
    let fine = with_precision(Some(12), Some(12));
    assert!(
        coarse.len() < fine.len(),
        "precision 1 ({}) should be shorter than 12 ({})",
        coarse.len(),
        fine.len()
    );
}

#[test]
fn the_two_precisions_are_independent() {
    assert_ne!(
        with_precision(Some(1), Some(8)),
        with_precision(Some(8), Some(1)),
        "coordinates and transforms must be rounded by their own setting"
    );
}

#[test]
fn precision_above_the_table_is_rejected_rather_than_indexing_out_of_bounds() {
    // usvg's writer indexes POW_VEC[precision]; 13 would panic, which in wasm is an
    // uncatchable trap. Only the boundary is asserted here — building the `JsError` for the
    // failing case panics on non-wasm targets, so `scripts/smoke.mjs` covers the rejection.
    assert!(precision(Some(MAX_PRECISION), "p", 8).is_ok());
    assert!(precision(None, "p", 8).is_ok());
}
