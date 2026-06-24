## Data Visualization Operation Manual
Use this prompt when the task is to analyze data visually, create charts, redesign visualizations, build dashboards, or turn datasets into an explanatory visual story.

- Start by understanding the dataset, grain, fields, units, missingness, transformations, and the user's decision or communication goal. Do not pick a chart type before identifying the comparison, distribution, relationship, ranking, part-to-whole, flow, geography, or time pattern that matters.
- Use structured parsers and data tools for CSV, JSON, spreadsheets, logs, databases, and other tabular formats. Keep data cleaning, aggregation, filtering, binning, date parsing, and unit conversion explicit and reproducible.
- Choose visualization libraries that match the deliverable. Prefer established charting systems such as Apache ECharts, Vega-Lite, Plotly, Observable-style notebooks, D3 where custom interaction is necessary, or the existing library already used by the project. Do not draw charts from scratch with raw SVG when a mature charting or diagramming library can produce the result more reliably.
- Design the visualization around the question, not decoration. Use direct labels where possible, readable axes, clear legends, honest scales, visible uncertainty, and chart annotations that explain the pattern without overstating causality.
- For dashboards, prioritize scanning, comparison, filtering, drill-down, and state clarity. Keep controls predictable, preserve stable layout dimensions, and avoid dense card grids when a table, chart matrix, tabs, or coordinated views would serve the workflow better.
- For data storytelling, put the key takeaway near the visual, show enough context for trust, and keep chart sequences ordered by reasoning rather than by dataset column order.
- Respect the parent visual and new-build manuals: the visual system must be polished and verified, and any new implementation must use clear module boundaries, typed or schema-validated data contracts, and maintainable project structure.

### Reverse-thinking workflow example:
- If the user asks for a chart or dashboard, first work backward from the decision, comparison, or insight the viewer should reach. Identify the viewer's role, the question they need answered, the fastest trustworthy path to that answer, and the possible misreadings to prevent; then choose the data transformation, chart type, annotation, interaction, and layout hierarchy. For example, if the user wants a revenue dashboard for operators, the goal is not visual richness; the goal is fast anomaly detection and action, so prioritize time trends, variance against target, segment drill-down, clear status color, compact tables for exceptions, and validation that every displayed number matches the transformed source data.

### Validation:
- Verify the rendered result with representative data, empty or sparse data, long labels, mobile and desktop widths when relevant, and interaction states such as filters, tooltips, selections, legends, and loading or error states.
- Cross-check chart values against the transformed data before finishing. Make sure scales, sorting, aggregation, percentages, labels, colors, and legends do not mislead or contradict the source data.
