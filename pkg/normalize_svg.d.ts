/* tslint:disable */
/* eslint-disable */

/**
 * Normalizes `svg` into the subset a partial SVG parser handles well: `<use>`, nested `<svg>`,
 * CSS, shape elements **and `<text>`** all become plain `<path>`.
 *
 * Text is outlined here rather than left to the caller precisely because the fonts are embedded:
 * resolving glyphs at this layer is what makes the output identical on every OS.
 *
 * Both precisions default to usvg's own (8) and accept 0-12. Lowering them shrinks the output
 * substantially, which matters when the consumer copies coordinates verbatim: a PDF exporter
 * writes them straight into the content stream, where full-precision floats left embedded-SVG-heavy
 * PDFs 85%-digits by weight. 3 is a good value there — far below visibility at any plausible
 * scale — but it is the caller's call to make, not this crate's.
 */
export function normalize(svg: string, coordinates_precision?: number | null, transforms_precision?: number | null): string;
