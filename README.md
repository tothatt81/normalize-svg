# normalize-svg

[![CI](https://github.com/tothatt81/normalize-svg/actions/workflows/ci.yml/badge.svg)](https://github.com/tothatt81/normalize-svg/actions/workflows/ci.yml)

Flattens an arbitrary SVG into a minimal, predictable subset: CSS, `<use>`/`<symbol>`, nested
`<svg>`, every shape element **and `<text>`** all become plain `<path>`. The output is geometry —
no fonts to resolve, no cascade to apply, no references to follow.

Built for renderers with a partial SVG parser — canvas libraries, PDF exporters, embedded viewers:
the ones that keep SVG vector rather than rasterising it, but understand only a fraction of the
spec. Feed one normalized SVG and the vector path stays intact.

```bash
npm install normalize-svg
```

> **Shipping this in a product? It embeds Twemoji artwork, which is CC-BY 4.0.** Free for
> commercial use, royalty-free and irrevocable — but attribution is a condition, and CC-BY 4.0
> §3(a)(1) wants more than a name. Paste this somewhere users can see it (about box, credits,
> docs); the full form and the reasoning are in [`NOTICE.md`](./NOTICE.md#attribution-you-can-paste).
>
> ```text
> Emoji artwork: Twemoji — © Twitter, Inc and other contributors;
> © jdecked/twemoji contributors (https://github.com/jdecked/twemoji).
> Licensed under CC BY 4.0: https://creativecommons.org/licenses/by/4.0/
> Modified: rebuilt as a COLR/CPAL font, glyph advances normalised, keycap
> ligatures added. Provided without warranties or conditions of any kind.
> ```

```js
const { normalize } = require("normalize-svg");

normalize('<svg xmlns="http://www.w3.org/2000/svg"><rect width="5" height="5"/></svg>');
// => '<svg ...><path d="M 0 0 L 5 0 L 5 5 L 0 5 Z"/></svg>'
```

It is a single synchronous function. Invalid SVG throws.

## Precision

```ts
normalize(svg: string, coordinates_precision?: number, transforms_precision?: number): string
```

Both default to usvg's own precision of 8 and accept **0–12**; anything outside that range throws
rather than reaching usvg, whose writer indexes a 13-entry table by this value and would panic.

Lower them when the consumer copies coordinates verbatim — a PDF exporter typically writes them
straight into the content stream (the image transform rides a separate `cm`), so source precision
is output precision, and full-precision floats left embedded-SVG-heavy PDFs 85%-digits by weight.
**3 is a good value there** — 0.001 user units is far below visibility at any plausible scale:

```js
normalize(svg, 3, 3); // ~19% smaller than the default on mixed shape-and-text content
```

That choice belongs to the caller rather than to this package, which is why the default is usvg's
and not 3.

## What it is

[usvg][] (the SVG simplifier from resvg) compiled to WebAssembly, plus two things upstream does
not do on its own:

- **Embedded fonts.** A text face and an emoji face are compiled into the `.wasm`, so `<text>` is
  outlined to the same glyphs on every OS — no font downloads, no fontconfig, no filesystem access
  at all. Resolving text here rather than leaving it to the caller is the whole point: it is what
  makes the output reproducible.
- **`@font-face` support.** SVGs carrying their own fonts as base64 `data:` URLs — including
  WOFF2, which is decompressed in-crate — have those faces registered before rendering.

Emoji work, including compound sequences: VS16, ZWJ families, flags, skin tones and keycaps all
shape as single clusters rather than tofu. Getting that right is most of why this package exists.

## Node only

The `.wasm` is loaded **synchronously** with `readFileSync`, so this is a CommonJS, Node-only
package. It does not work in a browser or in an edge runtime. `main` points at the wasm-bindgen
output, and the `.wasm` must sit next to the `.js` — if you copy files around at build time, copy
both.

## Fonts

Two faces are compiled in, which is most of the ~3.5 MB:

| Face        | Role                          | License                            |
| ----------- | ----------------------------- | ---------------------------------- |
| **Arimo**   | every family with no match    | OFL 1.1                            |
| **Twemoji** | emoji, incl. `Segoe UI Emoji` | CC-BY 4.0 (art), Apache-2.0 (tool) |

Fallback is deliberately a visible tofu box rather than a silent OS font, so a coverage gap shows
up instead of rendering differently per machine.

Both are also shipped as loose files, for callers that need the identical faces at runtime — a
canvas renderer drawing text beside these SVGs, say, so both compose emoji from the same face:

```js
const twemoji = require.resolve("normalize-svg/assets/twemoji.ttf");
const ranges = require("normalize-svg/assets/twemoji.ranges.json");
```

`twemoji.ranges.json` is the sidecar listing the codepoint ranges the emoji face covers, for
callers doing their own face selection.

### Which Arimo to load

`assets/Arimo.ttf` is a **variable** font with one `wght` axis, 400–700, defaulting to 400. That
default instance is what the `.wasm` renders unless the SVG asks otherwise: usvg maps `font-weight`
onto the axis, so `<text font-weight="700">` yields a genuinely interpolated bold rather than a
synthesised one.

For embedding into PDF, load `assets/Arimo-Regular.ttf` instead:

```js
const arimo = require.resolve("normalize-svg/assets/Arimo-Regular.ttf");  // PDF
const arimoVF = require.resolve("normalize-svg/assets/Arimo.ttf");        // needs 400–700
```

**PDF cannot express a variable font.** The format has no way to name an instance along an axis, so
a producer has to pin one before embedding; a variable font is not something it can pass through.
What differs between producers is only the fallback when you hand them one — embed the raw sfnt and
let readers draw the default master, subset it badly, or give up on embedding a font program
altogether. Pin the axis yourself and the question never arises, which is what
`Arimo-Regular.ttf` is for.

Worth knowing that the last fallback is not hypothetical. Measured with [`skia-canvas`][skia-canvas]
3.0.8, one line of 32px text:

| Loaded face         | PDF font                     | Embedded program        | Size    |
| ------------------- | ---------------------------- | ----------------------- | ------- |
| `Arimo.ttf` (VF)    | `/Type3`, 20 `CharProcs`     | none — glyphs as streams | 10.4 KB |
| `Arimo-Regular.ttf` | `/Type0` → `/CIDFontType2`   | `/FontFile2`, 3.6 KB subset | 4.6 KB |

Type 3 still extracts as text — `/ToUnicode` is written either way — but there is no hinting, no
glyph reuse, and the cost grows with every distinct glyph drawn: on a realistic paragraph at eight
sizes the same page is **32.8 KB against 8.9 KB**, 3.7× larger. Check your own producer rather than
assuming it does better.

The static instance is that same Regular with the axis pinned and `fvar`/`gvar`/`HVAR` dropped:
321 KB against 496 KB, and outline-identical — all 3301 glyphs, advances and vertical metrics
compare equal. That is what makes it embeddable as a plain subsetted TrueType.

The trade is that it is Regular only. If you need weights above 400, use the variable file and set
the axis yourself. Regenerate with:

```bash
# 1777284247 is Arimo.ttf's own head.modified, so the instance inherits its source's date
# rather than the build clock — without it fontTools stamps now, and the output stops
# matching the committed file byte for byte.
SOURCE_DATE_EPOCH=1777284247 fonttools varLib.instancer assets/Arimo.ttf wght=400 \
  -o assets/Arimo-Regular.ttf
```

> **`assets/twemoji.ttf` is a build artifact, not a stock download.** Its advance is normalised to
> **1.15em** — canvas offers no way to override an advance per glyph, so the number has to live in
> the font — and it carries a local patch adding short-form keycap ligatures. Upstream Twemoji
> defines only `<base> U+FE0F U+20E3 → keycap`, but Chrome and Skia both consume the U+FE0F
> variation selector before shaping, so HarfBuzz is handed `<base> U+20E3`, matches nothing, and
> `0️⃣`–`9️⃣`/`#️⃣`/`*️⃣` render as two blank glyphs — silently, at the right width. The patch adds 12
> rules to the existing `ccmp` lookup, one per keycap base, each with the single component `U+20E3`
> and the same `LigGlyph` as the long form, appended after it so the longer match stays first.
>
> If you ever regenerate this font, **re-apply both**, then verify:
>
> ```bash
> hb-shape --font-file=assets/twemoji.ttf --unicodes=0033,20E3   # must be one u33_fe0f_20e3
> hb-shape --font-file=assets/twemoji.ttf --unicodes=0033        # must stay one blank glyph
> ```

## The usvg pin

`Cargo.toml` pins `usvg` to a **git rev of a third-party fork** — `StefanoD/resvg` rev `cf580845`
— carrying the unmerged PR [linebender/resvg#1087], which fixes font fallback for compound emoji.

Without it, upstream re-shapes a span with the fallback face and merges the two glyph vectors _by
index_, bailing out entirely when the lengths differ. A multi-codepoint cluster ligates to one
glyph in the emoji font while the text font leaves one `.notdef` per codepoint, so the counts
diverge and every fallback glyph is dropped — `Hi ❤️` renders the emoji as tofu. See
[linebender/resvg#861], open since 2024-12.

The pin is by rev, not branch: the fork behind an earlier attempt at this fix was deleted and took
its PR with it. It is also pinned to 0.47.0 (rustybuzz) rather than 0.48.x (harfrust), which is
where that PR was cut from.

Revisit when #1087 lands upstream, then move back to a crates.io release.

## Why `pkg/` is committed

Because of the pin above, building from source puts a third-party fork that **has already vanished
once** on the critical path. So the `wasm-pack` output is committed and consumers — and CI — need
no Rust toolchain.

`src/`, `Cargo.toml` and `Cargo.lock` are here so the artifact can be audited and rebuilt.

## Building

Needs Docker and a Rust toolchain:

```bash
npm run build        # rebuild pkg/ in the pinned container — this is the artifact
npm run build:native # same flags, host toolchain — fast iteration, do not commit the result
npm run test         # cargo test — runs the crate's own suite natively
npm run smoke        # exercise the built .wasm through the public API
npm run verify       # build + no-diff against committed pkg/ + test + smoke
```

**The build environment is part of the artifact.** The same source and the same rustc produce a
different `.wasm` on macOS than on Linux — the two toolchains ship separately compiled `wasm32`
std, so the builds reference different std functions and the binary differs by a few dozen bytes.
Neither is wrong, but only one can be the committed one, so `npm run build` runs inside a container
pinned by digest in `scripts/build.sh` and that output is canonical. `build:native` is for
iterating; its bytes will not match, which is why `verify` never uses it.

Commit the regenerated `pkg/`. `wasm-pack` also writes its own `pkg/package.json` — inert, since
this package publishes from the root manifest, whose `files` list names the four `pkg/` artifacts
individually — plus a `pkg/.gitignore` containing `*` and copies of the README and LICENSE. The
build script deletes those three, since this repo commits `pkg/` deliberately.

Forgetting to commit that rebuild is the failure this package is most exposed to: consumers would
run a `.wasm` that no longer matches `src/`, and nothing in the repo would say so. `npm run verify`
rebuilds and fails on any diff against the committed `pkg/`. CI runs it on every push, and
`prepublishOnly` runs it again at publish time — because publishing reads the working directory,
not the pushed branch, so CI alone cannot catch a publish from a dirty tree.

`.cargo/config.toml` raises the wasm stack to 8 MB, which is load-bearing rather than tuning:
on the 1 MB default, deeply nested input overflows the stack before usvg's own depth guard can
fire, and a wasm stack overflow is an uncatchable trap that poisons the module instance for
every subsequent call.

A `RUSTFLAGS` environment variable overrides that file wholesale and silently restores the 1 MB
stack. Both build paths must set one, so both repeat the stack flag verbatim — `scripts/build.sh`
and the `build:native` script in `package.json`. Change it in one place and you must change it in
all three; the smoke suite checks the resulting depth rather than the flags, so a mismatch fails
loudly instead of shipping.

The rest of what they set is two `--remap-path-prefix` flags, which keep the build environment's
absolute paths out of the released binary; without them the wasm carries 154 of them and differs
between machines built from identical source. `trim-paths` would say the same thing in one line but
is not stable as of Cargo 1.97. Build by hand with those same flags, or with none at all — never
with a partial set.

## License

`MPL-2.0 AND OFL-1.1 AND CC-BY-4.0`

The **code** (`src/`, and therefore `LICENSE`) is **MPL-2.0**, marked per-file. MPL is file-level
copyleft: linking against this package does not reach your own code.

The declared expression is wider than that because the shipped `.wasm` is a **combined work** —
it statically links usvg and embeds two fonts, so every copy carries all three sets of terms:

| Embedded               | License        | What it obliges a redistributor to do        |
| ---------------------- | -------------- | -------------------------------------------- |
| usvg (linked)          | MPL-2.0        | make source obtainable — this repo, pinned rev |
| Arimo (font)           | OFL-1.1        | keep the notice; never sell the font alone   |
| Arimo-Regular (instance) | OFL-1.1      | same, plus §3 — a Modified Version, see `NOTICE.md` |
| **Twemoji art** (font) | **CC-BY-4.0**  | **attribute the creators**                   |

Ready-to-paste attribution is at the top of this file; the full picture is in `NOTICE.md`.

Two notes on the expression. It uses SPDX `AND`, which describes the **artifact** rather than each
file — it does not mean your code is CC-BY. And Twemoji's Apache-2.0 is deliberately absent: that
covers the build scripts which assembled the font, and this package ships their output, not them.

> If the CC-BY attribution requirement is unwelcome downstream, the way to remove it is to remove
> the artwork — swapping Twemoji for an OFL-licensed emoji face would make this package
> `MPL-2.0 AND OFL-1.1`, with no attribution riding on consumers. That is a real piece of work,
> not a metadata change: the 1.15em advance and the keycap-ligature patch would both need
> reproducing, and anything pairing this with a canvas renderer would need the same face on
> both sides to stay consistent.

[CC-BY 4.0]: https://creativecommons.org/licenses/by/4.0/

[skia-canvas]: https://skia-canvas.org
[usvg]: https://github.com/linebender/resvg/tree/main/crates/usvg
[wasm-pack]: https://rustwasm.github.io/wasm-pack/
[linebender/resvg#1087]: https://github.com/linebender/resvg/pull/1087
[linebender/resvg#861]: https://github.com/linebender/resvg/issues/861
