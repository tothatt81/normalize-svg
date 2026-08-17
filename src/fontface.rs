// This Source Code Form is subject to the terms of the Mozilla Public License,
// v. 2.0. If a copy of the MPL was not distributed with this file, You can
// obtain one at https://mozilla.org/MPL/2.0/.
//
// The fonts under `assets/` are NOT covered by this notice — see NOTICE.md.

//! Reading the fonts an SVG carries in its own `@font-face` rules.
//!
//! usvg ignores `@font-face` entirely — its `<style>` handling goes through `simplecss`, which
//! parses rule sets and skips at-rules — so without this an Excalidraw export would render its
//! hand-drawn faces in whatever we happened to bundle.

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;

/// Fonts the SVG carries in its own `@font-face` rules, as `(family, sfnt bytes)`.
///
/// Best-effort throughout: anything unreadable is skipped and that family falls back like any
/// unknown name, because a broken font must never fail the render. Only `data:` URLs are
/// honoured, since wasm cannot fetch.
pub(crate) fn embedded_fonts(svg: &str) -> Vec<(String, Vec<u8>)> {
    // `allow_dtd` is load-bearing: roxmltree rejects a DOCTYPE by default, and Excalidraw's
    // exports open with the SVG 1.1 doctype.
    let options = roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let Ok(doc) = roxmltree::Document::parse_with_options(svg, options) else {
        return Vec::new(); // usvg is about to reject it too, with a better message
    };

    let mut fonts = Vec::new();
    for style in doc.descendants().filter(|n| n.has_tag_name("style")) {
        for text in style.children().filter_map(|n| n.text()) {
            for block in font_face_blocks(text) {
                let (Some(family), Some(src)) = (declaration(block, "font-family"), declaration(block, "src"))
                else {
                    continue;
                };
                if let Some(bytes) = decode_src(src) {
                    fonts.push((family.to_string(), bytes));
                }
            }
        }
    }
    fonts
}

/// The `{ … }` body of every `@font-face` rule in a stylesheet, found by brace matching.
fn font_face_blocks(css: &str) -> Vec<&str> {
    let mut blocks = Vec::new();
    let mut rest = css;
    while let Some(at) = rest.find("@font-face") {
        rest = &rest[at + "@font-face".len()..];
        let Some(open) = rest.find('{') else { break };
        let Some(close) = rest[open..].find('}') else { break };
        blocks.push(&rest[open + 1..open + close]);
        rest = &rest[open + close..];
    }
    blocks
}

/// The value of one declaration inside a `@font-face` body, trimmed.
///
/// Splits on semicolons *outside* parentheses: a `src` carries `url(data:font/woff2;base64,…)`,
/// and a naive `split(';')` cuts that data URL in half.
fn declaration<'a>(block: &'a str, property: &str) -> Option<&'a str> {
    let mut depth = 0usize;
    let mut start = 0usize;
    let mut decls = Vec::new();
    for (i, ch) in block.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ';' if depth == 0 => {
                decls.push(&block[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    decls.push(&block[start..]);

    decls
        .into_iter()
        .filter_map(|decl| decl.split_once(':'))
        .find(|(name, _)| name.trim().eq_ignore_ascii_case(property))
        .map(|(_, value)| value.trim())
}

/// The first `url(data:…;base64,…)` in a `src`, decoded to sfnt bytes.
///
/// woff2 is what Excalidraw emits and `fontdb` cannot read it, so decompression is tried
/// unconditionally and the original bytes used when it declines — the decoder validates the
/// header up front, a more honest format check than sniffing `wOF2` ourselves.
fn decode_src(src: &str) -> Option<Vec<u8>> {
    for chunk in src.split("url(").skip(1) {
        let Some((raw, _)) = chunk.split_once(')') else {
            continue; // an unterminated url(, so not a source we can read
        };
        let Some((_, b64)) = raw.trim().trim_matches(['"', '\'']).split_once(";base64,") else {
            continue; // a plain-text data: URL, or a remote/local() source we cannot fetch
        };
        let Ok(bytes) = BASE64.decode(b64.trim().as_bytes()) else {
            continue;
        };
        return Some(woff2::convert_woff2_to_ttf(&mut bytes.as_slice()).unwrap_or(bytes));
    }
    None
}

#[cfg(test)]
mod tests;
