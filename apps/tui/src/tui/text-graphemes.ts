const segmenter =
  typeof Intl !== "undefined" && "Segmenter" in Intl
    ? new Intl.Segmenter(undefined, { granularity: "grapheme" })
    : undefined;

export function graphemes(value: string): string[] {
  if (!value) return [];
  return segmenter ? [...segmenter.segment(value)].map((item) => item.segment) : Array.from(value);
}

export function graphemeBoundaries(value: string): number[] {
  const boundaries = [0];
  let offset = 0;
  for (const grapheme of graphemes(value)) {
    offset += grapheme.length;
    boundaries.push(offset);
  }
  return boundaries;
}
