Use `web_discover` to find public website text, media, and reusable assets for web pages or interactive experiences.

Prefer `website` for source pages and docs, `image` for visual references, `video`/`audio` for media, and `asset` for shaders, textures/materials, 2D sprites/UI assets, 3D models, and sound effects.

`asset` searches multiple asset sources, including polydown/Poly Pizza, Magic UI, shadcn/ui, Objaverse, ambientCG, Sketchfab download API when credentials are available, Freesound, Internet Archive, Kenney, OpenGameArt, and Poly Haven. Results are saved by default under the workspace `.tura/media` directory unless `--download-dir` is provided. Direct asset files are downloaded automatically; zip archives are saved and extracted into subtype folders.

You should inspect downloaded files before using them. For visual/media downloads, pass the same `--download-dir` to `read_media` in a later step of the same batch when review is needed.

NEVER put multiple search goals in one command line; use multiple command lines in a batch.

Input is CLI text:

```text
website "OpenAPI docs" --max-results 3
website "https://example.com/page" --download-dir docs
image "https://example.com/a.jpg https://example.com/b.webp" --download-dir media/images
image "portrait official" --max-results 10 --download-dir media/image --min-size 10000
video "performance clip" --max-results 1 --download-dir media/video --format "best[height<=540][ext=mp4]/best[height<=540]/best"
audio "bell sound" --max-results 1 --download-dir media/audio --format "bestaudio/best"
asset 3d "space cruiser glb" --max-results 2 --download-dir assets
asset texture "brushed metal material" --max-results 2 --download-dir assets
asset shader "scanline hud wgsl" --max-results 2 --download-dir assets
asset 2d "cockpit ui sprite" --max-results 2 --download-dir assets
asset audio "laser impact sfx" --max-results 2 --download-dir assets
```

Arguments:
- Type: `website`, `image`, `video`, `audio`, or `asset`. You may pass it as the first word or with `--type`.
- Asset type: for `asset`, pass `shader`, `texture`, `2d`, `3d`, `audio`, or `auto` as the next word or with `--asset-type`.
- Query: pass quoted search text, a single webpage URL, multiple direct media URLs, one or more direct asset URLs, or `--query`.
- Query text is literal. Do not encode filters as words inside the query; use explicit CLI arguments such as `--include-regex` and `--exclude-regex`.
- For remote model/API or media-generation work, assume model/API knowledge may be stale: search official current docs and model/version pages first, using recent year+month terms instead of "latest", and save relevant docs under `doc/`.
- For image and website tasks, start with a short search query to find candidate pages and media URLs, then fetch or download from the relevant result.
- For direct media or asset downloads, pass one or more URLs as the quoted query.
- `--max-results N`: result limit. In `asset` mode, all relevant source families are searched, then merged and capped.
- `--download-dir DIR`: override the save directory. If omitted, results are saved under the workspace `.tura/media` directory.
- `--max-results` is also the maximum number of files/pages/assets saved. Use `--max-results 1` when you need one file or page.
- Asset downloads are placed under `DIR/<asset_type>/`. Zip archives are kept in `DIR/<asset_type>/archives/` and extracted beside them.
- `--min-size BYTES` / `--max-size BYTES`: filter downloaded media and asset files.
- `--format SELECTOR`: yt-dlp format selector for video/audio downloads. Video defaults to one 540p-or-lower file; avoid split video+audio selectors unless the user explicitly asks.
- `--include-regex REGEX` / `--exclude-regex REGEX`: filter result title, URL, and snippet.

Output:
- Without `--download-dir`: website, media, and asset results are saved under the workspace `.tura/media` directory.
- With `--download-dir`: website saves one cleaned `.md` per page, media saves downloaded files, and asset saves downloaded files plus extracted archive contents under the provided directory.
- Use `site:domain` in `website` queries for focused searches. `image`, `video`, `audio`, and `asset` treat it as ordinary search text unless the asset source wrapper adds its own site filters.
