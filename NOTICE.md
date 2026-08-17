# Third-party notices

`normalize-svg` ships a prebuilt `.wasm`. That binary is a combined work: it statically links
third-party Rust crates and **embeds two font files**. The obligations below travel with the
package — anyone redistributing it, or a bundle containing it, carries them too.

This is why the package declares `MPL-2.0 AND OFL-1.1 AND CC-BY-4.0` rather than just the licence
of its own code. The SPDX `AND` describes the shipped artifact, not any single file: the code is
MPL-2.0, and the other two arrive with the embedded fonts.

Twemoji's Apache-2.0 is **not** in that expression. It covers the build scripts that assembled the
font; this package distributes their output, not the scripts themselves.

## usvg / resvg — MPL-2.0

The normalizer is a thin wrapper around [`usvg`](https://github.com/linebender/resvg), copyright
the resvg authors, licensed under the Mozilla Public License 2.0. Its object code is linked into
`pkg/normalize_svg_bg.wasm`.

MPL-2.0 is file-level copyleft: you may distribute the compiled form, provided recipients can
obtain the source of the MPL-covered files. This package is itself MPL-2.0 (see `LICENSE`), and
`Cargo.toml` pins the exact upstream revision, so that obligation is discharged by the repository.

Note the pin is to a **fork** — `StefanoD/resvg` rev `cf580845` — carrying the unmerged PR
[linebender/resvg#1087]. See "The usvg pin" in the README for why.

## Embedded fonts

Both faces are compiled into the `.wasm` and are therefore redistributed with every copy of this
package. They are also shipped as loose files under `assets/` for callers that need the same faces
at runtime.

### Arimo — SIL Open Font License 1.1

Copyright 2020 The Arimo Project Authors (https://github.com/googlefonts/arimo). Designed by Steve
Matteson; metric-compatible with Arial and Liberation Sans. Full text: `assets/ARIMO-LICENSE.md`.

Used as the fallback face, so a coverage gap renders a visible tofu box rather than silently
picking an OS font. Two copies ship, and they are not the same file:

| File                       | Form                        | OFL status                    |
| -------------------------- | --------------------------- | ----------------------------- |
| `assets/Arimo.ttf`         | variable, `wght` 400–700    | unmodified v1.341             |
| `assets/Arimo-Regular.ttf` | static instance at `wght`400 | **Modified Version** under §3 |

The variable font is the upstream binary, byte for byte, and is the one compiled into the `.wasm`.
The static file is derived from it by `fonttools varLib.instancer` (see the README), which pins the
axis and drops `fvar`, `gvar` and `HVAR`; all 3301 glyph outlines, advances and vertical metrics are
identical to the variable font at its default instance.

Instancing makes that file a Modified Version, so OFL §3 **is** engaged for it. Its conditions are
met: Arimo declares no Reserved Font Name, so the derived file may keep the name; it stays under
OFL 1.1 with `assets/ARIMO-LICENSE.md` alongside; and it is not sold on its own. The copyright and
licence notices in its `name` table (IDs 13 and 14) are carried through from the original.

OFL §2 expressly permits bundling the font with software, provided each copy carries the copyright
notice and the licence — which is what `assets/ARIMO-LICENSE.md` is for. Note the font is also
compiled into the `.wasm`, so that condition applies to the binary too, not only to the loose file.

> Two Arimo lineages exist and they are **not** under the same licence: the 2010-2012 Chrome OS
> croscore release was Apache-2.0, while the current googlefonts/arimo project is OFL 1.1. This
> package ships the latter. Verified against name IDs 0, 13 and 14 of the binary itself rather
> than assumed from the older sidecar file.

### Twemoji — CC-BY 4.0 (graphics), Apache-2.0 (build tooling)

Copyright (c) Twitter, Inc and other contributors.
Copyright (c) jdecked/twemoji contributors.

`assets/twemoji.ttf` is a COLR/CPAL v0 build of the Twemoji artwork, produced with
[twemoji-colr](https://github.com/win98se/twemoji-colr) against the SVGs of
[jdecked/twemoji](https://github.com/jdecked/twemoji) v17.0.2.

**The graphics are CC-BY 4.0 and require attribution.** If you redistribute this package, or any
artifact containing its `.wasm`, you must credit the copyright holders above. Full text:
`assets/TWEMOJI-LICENSE.md`.

The font is a build artifact rather than a stock download — its advance is normalised to 1.15em
and it carries a local keycap-ligature patch. See "Fonts" in the README before regenerating it.

## Attribution you can paste

CC-BY 4.0 is **royalty-free, irrevocable and explicitly fine for commercial and paid products**
(§2(a)(1)). Attribution is the only condition — but §3(a)(1) asks for six elements, not just a
name, and the last one is easy to miss because it depends on what *this* package did to the font:

| §3(a)(1) wants                | Supplied by                                              |
| ----------------------------- | -------------------------------------------------------- |
| identification of the creators | "Twitter, Inc and other contributors; jdecked/twemoji …"  |
| a copyright notice             | the `©` marks                                             |
| a notice referring to the licence | "Licensed under CC BY 4.0"                             |
| a notice referring to the disclaimer | "Provided without warranties …"                    |
| a URI to the material          | the jdecked/twemoji link                                  |
| **indication of modification** | **"Modified: rebuilt as COLR/CPAL, advances normalised, keycap ligatures added"** |

```text
Emoji artwork: Twemoji — © Twitter, Inc and other contributors;
© jdecked/twemoji contributors (https://github.com/jdecked/twemoji).
Licensed under CC BY 4.0: https://creativecommons.org/licenses/by/4.0/
Modified: rebuilt as a COLR/CPAL font, glyph advances normalised, keycap
ligatures added. Provided without warranties or conditions of any kind.
```

"Reasonable to the medium" is the standard, so an about box, a credits screen or a licences page
all qualify. It does **not** have to appear next to each emoji.

> This discharges CC-BY and nothing else. It is not a substitute for the OFL notice travelling
> with Arimo, nor for MPL-2.0 source availability if you distribute binaries — see above.

## Test-only

`assets/AND-Regular.otf` (SIL Open Font License 1.1, copyright 2016-2019 Adobe) and the files
under `tests/fixtures/` are used by `cargo test` only. They are **not** embedded in the `.wasm`
and are **not** published to npm.

[linebender/resvg#1087]: https://github.com/linebender/resvg/pull/1087
