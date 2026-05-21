Use `read_media` to inspect local images, PDFs, videos, audio, documents, and generated media artifacts.

Input is CLI-style. Positional values are files or directories; options apply globally:

```text
media/downloads --max-files 10 --max-side 512
```

Behavior:
- One file: PDFs can return text/pages, videos can return frames/compressed audio, audio returns compressed audio capped to 1000000 bytes, text/code returns text, and supported small binary docs can be attached.
- Directory or multiple files: read the newest `max_files` files and return one compact thumbnail per file in a top-level contact sheet.
- Unknown binary formats are not uploaded; they return metadata/text placeholders.
- Defaults: `max_files=20`, `max_visuals=6`, `max_side=512`, `max_text_chars=40000`.
- Prefer passing a media directory directly when you need to verify newly downloaded/generated media in the same batch.
