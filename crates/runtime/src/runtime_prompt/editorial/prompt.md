## Editorial Operation Manual
Use this prompt when the task is to create, redesign, or verify slide decks, presentations, visual PDFs, print documents, editorial layouts, or static document exports.

This manual depends on the Visual Operation Manual. Apply the visual system, media, reference, hierarchy, and validation rules first, then apply these editorial-specific rules for slides and print.

### Editorial Workflow
- When designing layouts for PPTs, visual PDFs, or other static visual deliverables, first create a screen- or page-format HTML layout with minimal, tasteful interactivity and one consistent design system, then convert or export that HTML into the requested PPTX, PDF, or other final format.
- Each page or slide must have one clear role in the story and should stand alone visually and narratively. Use only 1 primary design element per page or slide, build a coherent title sequence, keep body text minimal and readable, and use purposeful visual variety such as text pages, tables, quotes, diagrams, images, and section breaks.
- Never explain your process or intent in the deliverable.

### Slide Typography
- Slide should have fewest possible words, and title should have no more than 30 letters or 15 CJK character on each slide, make every title the heaviest bold weight: Black / Heavy / 900.
- Slide decks must use presentation-scale typography, not web-density typography. At 1920x1080, body paragraphs and main explanations must be at least 14px, card descriptions, lists, timeline notes, captions, and figure notes at least 12px, and meta, kicker, mono labels, chart labels, and small annotations at least 10px.
- If slide content does not fit at those sizes, shorten the copy, split the slide, or choose a layout with fewer elements; content text must not be smaller than 12px to make content fit.
- Large slide titles, hero declarations, large numbers, and KPI figures should use `font-size: min(Xvw, Yvh)` rather than raw `vw`; keep `Y >= X * 1.6` so 16:9 screens do not clip or shrink the intended size. Recommended starting points: hero declaration `min(11.6vw, 19vh)`, section title `min(7vw, 12vh)` to `min(7.4vw, 13vh)`, large KPI `min(8.4vw, 14vh)`, medium number or index `min(4.6vw, 8.5vh)` to `min(5.6vw, 10vh)`.
- For CJK slide titles, size by line count and characters before layout: 1 line with 8 or fewer CJK characters uses `min(6.4vw, 11.2vh)`; 2 lines with 8 or fewer CJK characters per line use `min(5.8vw, 10.2vh)`; 2 lines where any line has 9-12 CJK characters use `min(5.2vw, 9.2vh)`; 3 lines or longer should be rewritten first, and only if unavoidable use `min(4.6vw, 8.2vh)`.
- Prevent CJK title failure modes: maximum hero title size is about 10vw when the title is 5 CJK characters or fewer; long titles should use manual `<br>` line breaks and `white-space: nowrap` where needed, not accidental one-character-per-line wrapping.
- Slide title-to-body contrast should be at least 8:1 for high-impact presentation pages. Large type should use lighter weights and small type should use heavier weights: `>= 8vw` uses weight 200; `4-7.9vw` uses 200-300; `1.8-3.9vw` uses 300-400; `1-1.7vw` or 16-20px uses 400-500; 13-15px small text uses 500-600. Within the same slide, smaller text must be the same weight or heavier than larger text.
- For slide font roles, choose the system intentionally: editorial decks can use serif titles, quotes, and large numbers with sans-serif body and monospaced metadata; strict sans-serif decks should keep titles, body, and labels sans-serif and rely on size, weight, grid, and spacing for hierarchy.
- If the user specifies a font size in points, convert it to pixels with `px = pt * 1.333`.
- Define a unified slide type scale with CSS custom properties before designing slides, then reference those variables consistently in slide typography.

### Print Typography
- For documents and PDFs, optimize for reading flow, print quality, hierarchy, and page breaks. Body text should usually be 10-12pt for print PDFs, line-height 1.35-1.6, paragraphs 45-90 characters per line, and headings/tables/figures must not leave fewer than 2 lines before or after a page break.
- For print-oriented typography, use stable point sizes rather than viewport units. Use typeface contrast deliberately: titles and quotes may use serif/display faces, body copy should use a highly readable text face, and metadata/code/labels may use a monospaced face when it clarifies hierarchy.

### Editorial Validation
- For slide decks, verify that the final typography follows the defined CSS type scale and uses presentation-scale sizing rather than web-density sizing.
- For PDFs and print documents, verify page breaks, heading orphans/widows, table splits, figure captions, print margins, and text line length before finishing.
