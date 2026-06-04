Use `read_media` to inspect local images, PDFs, videos, audio, documents, and generated media artifacts.

Input is CLI-style. Positional values are files or directories; options apply globally:

```text
media/downloads --max-files 10 --max-side 512
```

Behavior:
- Reads files or directories and returns compact visual/text evidence suitable for inspection.
- Defaults: `max_files=20`, `max_visuals=6`, `max_side=512`, `max_text_chars=40000`.
- Prefer passing a media directory directly when you need to verify newly downloaded/generated media in the same batch.
- For frontend or design work, use `read_media` on provided or captured screenshots to confirm layout, visual fit, and design issues.
