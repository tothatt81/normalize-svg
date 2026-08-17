Copyright (c) Twitter, Inc and other contributors.
Copyright (c) jdecked/twemoji contributors.

`twemoji.ttf` is a COLR/CPAL v0 build of the Twemoji artwork, produced from
https://github.com/win98se/twemoji-colr (a maintained fork of Mozilla's
twemoji-colr) against the SVGs of https://github.com/jdecked/twemoji at v17.0.2,
the community-maintained continuation of Twitter's archived Twemoji project.

Two licenses apply to it:

- **Graphics** (every glyph in the font) are licensed CC-BY 4.0:
  https://creativecommons.org/licenses/by/4.0/
  Use of the artwork requires attribution to the copyright holders above.

- **Build scripts** (the twemoji-colr tooling that assembled the font) are
  licensed Apache-2.0: http://www.apache.org/licenses/LICENSE-2.0

The same artwork, under the same CC-BY 4.0 terms, is used on the canvas side of
this renderer via `@twemoji/svg` — see `src/twemoji.ts`.
