# tura_path

`tura_path` is the single source of truth for instance / home / path resolution
across the Tura backend. It replaces the previously duplicated helpers
(`find_repo_root`, `my_root`, `default_db_dir`, `strip_verbatim_prefix`,
`normalize_workspace`) that had drifted across the gateway, router, and
session_log crates.

## instance_home model

An **instance_home** is one isolated Tura instance, modelled on codex's
`CODEX_HOME`. Everything per-instance derives from one `instance_home()`:

| Helper | Derives | Purpose |
|---|---|---|
| `instance_home()` | the home directory | isolation unit (dev / release / profile) |
| `home_runtime_dir()` | `<home>/.tura` | per-instance runtime state |
| `home_socket(name)` | `<home>/.tura/sockets/<name>.sock` | control endpoint address |
| `locks_dir()` | `<home>/.tura/locks` | flock files |
| `home_db_dir()` | private DB directory | session_db index and write queue |

Home selection precedence: `TURA_HOME` → repo root (in a source checkout) →
canonical current directory. dev / release / debug are simply **different
homes** selected by `TURA_HOME`, so they coexist with no shared ports or locks.

## Version handshake

`instance_version()` returns `"<pkg-version>+<build-kind>"` (`build_kind()` reads
the compile-time `TURA_BUILD_KIND`, defaulting to `dev`). Clients compare this
against the version reported by a per-home service on connect; a mismatch means
the client is talking to a different build and must refuse or restart it.

## Normalization

`normalize_path` canonicalizes and strips the Windows verbatim (`\\?\`, including
`\\?\UNC\`) prefix, so homes that differ only by case, trailing separator, or a
symlink hop resolve to the same instance. `normalize_workspace` canonicalizes a
session's workspace key (forward slashes, trimmed trailing separators, bare
drive/`/` roots preserved).

## Consumers

- `session_log::path` re-exports `default_db_dir` / `repo_root_from` /
  `normalize_workspace` from here.
- `gateway` resolves its reported project root via `canonical_root()` and its
  verbatim stripping via `strip_verbatim_prefix`.

> Binary-location fallbacks (`target/{debug,release}` lookups) are a separate
> concern, consolidated in later stages, and intentionally not owned here.
