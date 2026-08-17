/**
 * Exercises the committed `.wasm` through the public API — no Rust toolchain involved.
 *
 * `cargo test` covers the crate's internals; this covers the thing that actually ships, which is
 * the part a bad rebuild or a botched `files` list would break. Run it before publishing.
 */
import { createRequire } from "node:module";
import { readFileSync } from "node:fs";

const require = createRequire(import.meta.url);
const { normalize } = require("../pkg/normalize_svg.js");

const svg = (body, extra = "") =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40"${extra}>${body}</svg>`;

let failures = 0;
const check = (name, fn) => {
  try {
    fn();
    console.log(`  ok  ${name}`);
  } catch (error) {
    failures += 1;
    console.error(`FAIL  ${name}\n      ${error.message}`);
  }
};
const assert = (condition, message) => {
  if (!condition) throw new Error(message);
};

check("flattens a shape to a path", () => {
  const out = normalize(svg('<rect width="5" height="5"/>'));
  assert(out.includes("<path"), "no <path> in output");
  assert(!out.includes("<rect"), "<rect> survived");
});

check("resolves <use> against <defs>", () => {
  const out = normalize(
    svg('<defs><circle id="c" cx="5" cy="5" r="3"/></defs><use href="#c" x="10"/>'),
  );
  assert(out.includes("<path"), "no <path> in output");
  assert(!out.includes("<use"), "<use> survived");
});

check("applies CSS from a <style> block", () => {
  const out = normalize(svg('<style>.r{fill:#ff0000}</style><rect class="r" width="5" height="5"/>'));
  assert(/#ff0000|red/i.test(out), "fill from CSS was lost");
});

check("outlines text to paths using the embedded face", () => {
  const out = normalize(svg('<text x="2" y="20" font-size="12">Hi</text>'));
  assert(!out.includes("<text"), "<text> was left unresolved");
  assert(out.includes("<path"), "text produced no path geometry");
  // Arimo's H and i, not a tofu box: a notdef would be a single rectangular contour.
  assert((out.match(/[ML] /g) ?? []).length > 10, "text outline is too simple to be real glyphs");
});

check("shapes emoji as clusters rather than tofu", () => {
  // A ZWJ family and a keycap: the two cases the usvg fork and the font patch exist for.
  // Each must ligate to ONE glyph — a broken fallback drops them or emits per-codepoint notdefs.
  const plain = normalize(svg('<text x="2" y="30" font-size="12">.</text>'));
  const emoji = normalize(
    svg('<text x="2" y="30" font-size="12">\u{1F468}‍\u{1F469}‍\u{1F467}3️⃣</text>'),
  );
  const curves = (s) => (s.match(/[CQ] /g) ?? []).length;
  assert(curves(emoji) > curves(plain) + 50, "emoji rendered as tofu or were dropped entirely");
});

check("rejects malformed input by throwing", () => {
  let threw = false;
  try {
    normalize("not an svg at all");
  } catch {
    threw = true;
  }
  assert(threw, "invalid SVG did not throw");
});

check("precision defaults to usvg's own and is tunable", () => {
  const frac = '<g transform="rotate(13)"><circle cx="5" cy="5" r="3"/></g>';
  assert(normalize(svg(frac)) === normalize(svg(frac), 8, 8), "omitting precision is not usvg's default");
  assert(
    normalize(svg(frac), 1, 1).length < normalize(svg(frac), 12, 12).length,
    "lower precision did not shorten the output",
  );
  assert(
    normalize(svg(frac), 1, 8) !== normalize(svg(frac), 8, 1),
    "the two precisions are not independent",
  );
});

// usvg's writer indexes a 13-entry table by precision, so 13+ would panic — a trap that
// poisons the module for every later call. This is the only place that rejection can be
// tested: constructing the JsError panics on non-wasm targets, so `cargo test` cannot.
check("rejects out-of-range precision instead of trapping", () => {
  for (const [c, t] of [[13, undefined], [undefined, 13], [255, 255]]) {
    let threw = false;
    try {
      normalize(svg('<rect width="5" height="5"/>'), c, t);
    } catch (error) {
      threw = true;
      assert(!(error instanceof WebAssembly.RuntimeError), `precision ${c}/${t} trapped the wasm`);
    }
    assert(threw, `precision ${c}/${t} was accepted`);
  }
  // And the module is still usable afterwards.
  assert(normalize(svg('<rect width="5" height="5"/>')).includes("<path"), "module was left broken");
});

check("ships the runtime assets named in package.json#files", () => {
  const manifest = JSON.parse(readFileSync(new URL("../package.json", import.meta.url), "utf8"));
  for (const entry of manifest.files) {
    if (entry.endsWith("/")) continue;
    readFileSync(new URL(`../${entry}`, import.meta.url));
  }
  const ranges = JSON.parse(
    readFileSync(new URL("../assets/twemoji.ranges.json", import.meta.url), "utf8"),
  );
  assert(Array.isArray(ranges) && ranges.length > 0, "twemoji.ranges.json is not a non-empty array");
});

// The 8 MB stack lives in two places — `.cargo/config.toml` and the RUSTFLAGS that `npm run build`
// sets, which override that file wholesale. Losing either silently restores wasm-ld's 1 MB default,
// where this input traps instead of converting, and a trap is unrecoverable: every later call fails
// too. Nothing else in the suite would notice, so check the depth rather than the flags.
check("converts past the depth a 1 MB stack traps at", () => {
  const nested = svg(`${"<g>".repeat(900)}<rect width="5" height="5"/>${"</g>".repeat(900)}`);
  assert(normalize(nested).includes("<path"), "no <path> from deeply nested input");
  assert(normalize(svg('<rect width="5" height="5"/>')).includes("<path"), "module was left broken");
});

console.log(failures === 0 ? "\nsmoke: all checks passed" : `\nsmoke: ${failures} failed`);
process.exit(failures === 0 ? 0 : 1);
