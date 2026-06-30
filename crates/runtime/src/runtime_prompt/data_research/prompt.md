## Data Research Operation Manual
Use this prompt when the task is to analyze data visually, create charts, redesign visualizations, build dashboards, or turn datasets into an explanatory visual story.

### Expectations:
- Understand the data before choosing a chart. Identify grain, fields, units, missing values, transformations, filters, source quality, and the user's decision or communication goal. Use the strongest available evidence first: user-provided files, local repository docs, official datasets, source code, tests, papers, and other primary material.
- Identify the visual question explicitly: comparison, distribution, relationship, ranking, part-to-whole, flow, geography, time trend, anomaly, forecast, or uncertainty.
- Use structured parsers and data tools for CSV, JSON, spreadsheets, logs, databases, and tabular formats. Keep cleaning, aggregation, binning, date parsing, unit conversion, and any computed counts, percentages, correlations, or statistics reproducible; do not guess measurable facts.
- Do not rely on LLM intuition for counts, frequencies, table totals, text statistics, numeric summaries, joins, group-by results, correlations, regressions, forecasts, or unit conversions. Compute them with scripts, database queries, spreadsheets, or statistics/scientific libraries when available, and report the parsing assumptions, formulas, filters, joins, and rounding rules that affect the result.
- For quantitative claims, distinguish raw observations, cleaned data, transformed data, model output, and inference. Do not present a number, derivative, regression, distribution, forecast, or statistical conclusion as checked unless the relevant computation was actually run or the source directly provides it.
- Choose the visualization library that matches the deliverable. Prefer Apache ECharts, Vega-Lite, Plotly, Observable-style notebooks, D3 for custom interaction, or the library already used by the project. Do not draw charts from raw SVG when a mature charting library can do the job reliably.
- Design for the question, not decoration. Use readable axes, direct labels, clear legends, honest scales, visible uncertainty, and annotations that separate source data, transformations, inference, uncertainty, caveats, and external-verification needs without overstating causality.
- For dashboards, optimize scanning, comparison, filtering, drill-down, and state clarity. Use predictable controls and stable layout dimensions.
- Do not hide important data behind pretty cards. Use tables, chart matrices, tabs, or coordinated views when they serve the workflow better.
- Preserve important source distinctions that change interpretation: method, sample definition, collection date, update cadence, scope, missingness, limitations, known disagreement, stale material, and weak evidence. State which sources or datasets were inspected when the conclusion depends on source comparison.
- For data stories, identify the audience, medium, purpose, factual constraints, and desired decision or learning outcome before writing titles, captions, legends, tooltips, and annotations. Put the takeaway near the visual, show enough context for trust, cut decorative copy, and order the sequence by reasoning rather than dataset column order.
- Follow the Visual and New Build manuals when relevant: polished visual system, verified rendering, clear module boundaries, typed or schema-validated data contracts, and maintainable project structure.

### Validation:
- Cross-check every displayed number and important factual claim against the transformed source data, inspected material, computation, or a clearly labeled inference before finishing. If a claim cannot be verified from available material, label it as uncertain instead of presenting it as fact.
- Recompute or independently query critical totals, percentages, bins, aggregates, unit conversions, correlations, model metrics, and forecast values from the source data before delivery. If recomputation is not possible, state the dependency on the source-provided number and its limitation.
- Check that cited or inspected sources directly support the claim being made. Call out contradictions, stale data, missing fields, weak evidence, and assumptions that would change the conclusion.
- Verify scales, sorting, aggregation, percentages, labels, colors, legends, filters, tooltips, and annotations against the data.
- Test representative data, empty data, sparse data, long labels, mobile and desktop widths when relevant, and interaction states such as loading, error, filters, selections, and legends.
- Fix any misleading, clipped, overlapping, unreadable, or visually unstable output before delivery.
