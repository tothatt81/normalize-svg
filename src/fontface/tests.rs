// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.
//
// The fonts under `assets/` are NOT covered by this notice — see NOTICE.md.

//! Tests for `@font-face` extraction.

use super::*;
use crate::tests::{NOTDEF_FONT, NOTDEF_WOFF2};

fn svg_with(style: &str) -> String {
    format!("<svg xmlns=\"http://www.w3.org/2000/svg\"><defs><style>{style}</style></defs></svg>")
}

fn rule(family: &str, mime: &str, font: &[u8]) -> String {
    format!(
        "@font-face {{ font-family: {family}; src: url(data:font/{mime};base64,{}); }}",
        BASE64.encode(font)
    )
}

/// The regression that made both embedded-font tests fail first time round: a data URL
/// contains `;base64,`, so splitting declarations on every `;` cuts the `src` in half.
#[test]
fn a_semicolon_inside_a_data_url_does_not_split_the_declaration() {
    let fonts = embedded_fonts(&svg_with(&rule("EmbeddedFace", "otf", NOTDEF_FONT)));
    assert_eq!(fonts.len(), 1);
    assert_eq!(fonts[0].0, "EmbeddedFace");
    assert_eq!(fonts[0].1, NOTDEF_FONT);
}

#[test]
fn a_woff2_source_comes_back_decompressed() {
    let fonts = embedded_fonts(&svg_with(&rule("EmbeddedFace", "woff2", NOTDEF_WOFF2)));
    assert_eq!(fonts.len(), 1);
    // Decompressed to sfnt: `OTTO` for CFF outlines, which is what the fixture carries.
    assert_eq!(&fonts[0].1[..4], b"OTTO");
    assert_ne!(fonts[0].1, NOTDEF_WOFF2, "should not be the woff2 bytes");
}

#[test]
fn every_rule_in_a_stylesheet_is_read() {
    let css = format!(
        "{}\n{}",
        rule("First", "otf", NOTDEF_FONT),
        rule("Second", "woff2", NOTDEF_WOFF2)
    );
    let fonts = embedded_fonts(&svg_with(&css));
    assert_eq!(
        fonts.iter().map(|(f, _)| f.as_str()).collect::<Vec<_>>(),
        ["First", "Second"]
    );
}

#[test]
fn sources_we_cannot_fetch_are_skipped() {
    for src in [
        "url(https://example.com/font.woff2) format('woff2')",
        "local(\"Some Installed Face\")",
        "url(data:font/woff2,not-base64-encoded)",
    ] {
        let css = format!("@font-face {{ font-family: F; src: {src}; }}");
        assert!(embedded_fonts(&svg_with(&css)).is_empty(), "{src} should be skipped");
    }
}

#[test]
fn a_rule_missing_a_declaration_is_skipped() {
    let css = "@font-face { font-family: NoSource; }";
    assert!(embedded_fonts(&svg_with(css)).is_empty());
}

/// Why this reads `<style>` elements rather than scanning the raw markup: `@font-face`
/// appearing in ordinary text is not a rule.
#[test]
fn font_face_outside_a_stylesheet_is_not_a_rule() {
    let svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\"><text>{}</text></svg>",
        rule("NotAFace", "otf", NOTDEF_FONT)
    );
    assert!(embedded_fonts(&svg).is_empty());
}

/// Excalidraw opens its exports with the SVG 1.1 doctype, and roxmltree rejects a DOCTYPE
/// unless told otherwise — which silently cost every embedded font in a real export.
#[test]
fn a_doctype_does_not_hide_the_stylesheet() {
    let svg = format!(
        "<?xml version=\"1.0\" standalone=\"no\"?>
         <!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\"          \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">
{}",
        svg_with(&rule("EmbeddedFace", "woff2", NOTDEF_WOFF2))
    );
    let fonts = embedded_fonts(&svg);
    assert_eq!(fonts.len(), 1, "the doctype must not hide the @font-face rules");
    assert_eq!(&fonts[0].1[..4], b"OTTO");
}

#[test]
fn markup_that_is_not_xml_yields_nothing_instead_of_panicking() {
    assert!(embedded_fonts("<svg><style>@font-face {").is_empty());
}

/// Pins the contract `decode_src` relies on: the decoder rejects non-woff2 bytes by
/// returning `Err`, so trying it unconditionally is safe. A panic here would be a wasm
/// trap, not a caught error.
#[test]
fn the_woff2_decoder_declines_plain_sfnt_bytes_instead_of_panicking() {
    let mut bytes = NOTDEF_FONT;
    assert!(woff2::convert_woff2_to_ttf(&mut bytes).is_err());
}
