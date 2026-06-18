Use `web_discover` to find public website text or public image/video/audio artifacts.
Use `website`, to find reliable source website that hosts media url and use `image`, `video`, or `audio` to download the media， and use `read_media` to review.
NEVER put two search gol in one command line always use multiple command_lines in a batch.

Input is CLI text:

```text
website "OpenAPI docs" --max-results 3
website "https://example.com/page" --download-dir docs
image "https://example.com/a.jpg https://example.com/b.webp" --download-dir media/images
website "official API docs" --max-results 3 --download-dir docs
image "portrait official" --max-results 3 --download-dir media/image --min-size 10000
video "performance clip" --max-results 1 --download-dir media/video --format "best[height<=540][ext=mp4]/best[height<=540]/best"
audio "public domain bell sound" --max-results 1 --download-dir media/audio --format "bestaudio/best"
```

Arguments:
- Type: `website`, `image`, `video`, or `audio`. You may pass it as the first word or with `--type`.
- Query: pass quoted search text, a single webpage URL, multiple direct media URLs, or `--query`.
- For remote model/API or media-generation work, assume model/API knowledge may be stale: search official current docs and model/version pages first, using recent year+month terms instead of "latest", and save relevant docs under `doc/`.
- For image and website tasks, start with short search query to find candidate pages and media URLs, and fetch or download from the relevant result.
- For direct media downloads, pass one or more URLs as the quoted query. This works for image, video, and audio.
- `--max-results N`: result limit.
- `--download-dir DIR`: save results. Omit it to return text/links only.
- With `--download-dir`, `--max-results` is also the maximum number of files/pages saved. Use `--max-results 1` when you need one media file or one page.
- `--min-size BYTES` / `--max-size BYTES`: filter downloaded media files.
- `--format SELECTOR`: yt-dlp format selector for video/audio downloads. Video defaults to one 540p-or-lower file; avoid split video+audio selectors unless the user explicitly asks.
- `--include-regex REGEX` / `--exclude-regex REGEX`: filter result title, URL, and snippet.

Output:
- Without `--download-dir`: website returns fetched cleaned text only; media returns links and metadata.
- With `--download-dir`: website saves one cleaned `.md` per page; media saves downloaded files and returns relative paths plus source metadata.
- Use `site:domain` in `website` queries for focused searches. `image`, `video`, and `audio` treat it as ordinary search text.
- After downloading media, pass the same `--download-dir` to `read_media` command in a later step of the same batch; set `--max-files` to the expected download count.
