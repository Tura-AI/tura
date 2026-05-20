Use `read_media` when the task depends on local PDFs, images, screenshots, videos, or generated media artifacts.

`command_line` must be a JSON object. Always use `paths`, even for one file:

```json
{"paths":["brief.pdf","profile.png"],"include_text":true,"max_text_chars":40000,"max_visuals":6}
```

Behavior:
- Images return compact metadata and a downscaled JPEG preview.
- PDFs return extracted text when available plus compressed page previews.
- Videos return sampled compressed frame previews.
- Use this to inspect or validate media deliverables. It is for local files, not web search.
- Prefer workspace-relative paths when the media is inside the session workspace. If the user provided an absolute path, pass it unchanged.
