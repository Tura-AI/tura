## Data Visualization Operation Manual
Use this prompt when the task is to analyze data visually, create charts, redesign visualizations, build dashboards, or turn datasets into an explanatory visual story.

### Expectations:
- Understand the data before choosing a chart. Identify grain, fields, units, missing values, transformations, filters, and the user's decision or communication goal.
- Identify the visual question explicitly: comparison, distribution, relationship, ranking, part-to-whole, flow, geography, time trend, anomaly, forecast, or uncertainty.
- Use structured parsers and data tools for CSV, JSON, spreadsheets, logs, databases, and tabular formats. Keep cleaning, aggregation, binning, date parsing, and unit conversion reproducible.
- Choose the visualization library that matches the deliverable. Prefer Apache ECharts, Vega-Lite, Plotly, Observable-style notebooks, D3 for custom interaction, or the library already used by the project. Do not draw charts from raw SVG when a mature charting library can do the job reliably.
- Design for the question, not decoration. Use readable axes, direct labels, clear legends, honest scales, visible uncertainty, and annotations that explain patterns without overstating causality.
- For dashboards, optimize scanning, comparison, filtering, drill-down, and state clarity. Use predictable controls and stable layout dimensions.
- Do not hide important data behind pretty cards. Use tables, chart matrices, tabs, or coordinated views when they serve the workflow better.
- For data stories, put the takeaway near the visual, show enough context for trust, and order the sequence by reasoning rather than dataset column order.
- Follow the Visual and New Build manuals when relevant: polished visual system, verified rendering, clear module boundaries, typed or schema-validated data contracts, and maintainable project structure.

### Validation:
- Cross-check every displayed number against the transformed source data before finishing.
- Verify scales, sorting, aggregation, percentages, labels, colors, legends, filters, tooltips, and annotations against the data.
- Test representative data, empty data, sparse data, long labels, mobile and desktop widths when relevant, and interaction states such as loading, error, filters, selections, and legends.
- Fix any misleading, clipped, overlapping, unreadable, or visually unstable output before delivery.
