# Binary-Verified CLI Port Matrix 10

This suite is a 10-task refactoring matrix for CLI ports. It uses the same rule
as the eza task: the agent receives a local source checkout plus a fixed
official reference CLI, then rebuilds the tool in the task's assigned target
language.

The harness runs `./compile.sh`, then compares the official reference CLI
against the rebuilt `./executable` on the same argv, stdin, files, and
environment. The observable result must match: exit status, stdout, and stderr.

Per-task metadata, reference versions, release lookup rules, and binary
comparison cases live in `tasks.json`. Task-local report code lives under
`report/`.

## Matrix Summary

LOC is counted from the local fixed reference checkout as nonblank lines in the
task's primary source-language files. The count excludes `.git`, package caches,
and common build output directories such as `target`, `dist`, `build`, `out`,
and `node_modules`. If the local checkout is unavailable, the report script uses
the current snapshot values below.

| Task | Source -> target | Source files | Source LOC | Harness coverage | Main covered behavior |
|---|---:|---:|---:|---:|---|
| `eza` | Rust -> Python | 69 | 15,726 | 46 cases; 12 success groups; 5 error groups | listing, hidden, long-view, time-fields, tree-recursion, sorting, display-modes, classify, filtering, size-format, path-display, stdin; errors: invalid-option-value, invalid-numeric-value, missing-required-value, missing-path, unknown-option |
| `ripgrep` | Rust -> TypeScript | 98 | 45,427 | 36 cases; 14 success groups; 5 error groups | metadata, file-discovery, literal-search, regex-search, case-matching, line-context, counts, output-format, globs-types, stdin-patterns, replace-output, file-match-modes, ignore-hidden, type-list; errors: invalid-regex, missing-path, unknown-option, missing-option-value, invalid-type |
| `fzf` | Go -> Rust | 73 | 19,821 | 27 cases; 13 success groups; 4 error groups | metadata, fuzzy-filter, exact-filter, case-matching, field-selection, ordering, query-output, nul-io, exit-status, literal-filter, header-output, select-one, walker-help; errors: invalid-nth, invalid-tiebreak, unknown-option, invalid-scheme |
| `yq` | Go -> Java | 224 | 26,604 | 25 cases; 13 success groups; 5 error groups | metadata, yaml-query, json-query, format-conversion, eval-all, null-input, expression-source, inplace-update, csv-query, selection-expression, assignment-expression, unwrap-scalar, encoding-conversion; errors: missing-field-exit, invalid-expression, missing-file, invalid-format, invalid-output-format |
| `prettier` | TypeScript -> Go | 582 | 8,816 | 29 cases; 12 success groups; 5 error groups | metadata, parser-format, style-options, check-list, write-mode, stdin-filepath, metadata-json, config-behavior, ignore-behavior, pragma-behavior, range-format, cache-mode; errors: parser-error, unknown-parser, missing-file, invalid-option-value, missing-config |
| `typescript` | TypeScript -> Python | 20,542 | 1,504,501 | 20 cases; 10 success groups; 5 error groups | metadata, noemit-check, compiler-options, project-config, emit-output, config-output, js-checking, source-map-output, jsx-option, list-files; errors: type-error, unknown-option, missing-project, invalid-option-value, invalid-module |
| `black` | Python -> Java | 274 | 118,593 | 22 cases; 10 success groups; 5 error groups | metadata, code-format, file-check, style-options, stdin-format, write-mode, target-version, range-format, quiet-verbose, exclude-behavior; errors: syntax-error, missing-file, invalid-option-value, version-mismatch, invalid-line-range |
| `pyflakes` | Python -> Go | 22 | 8,209 | 12 cases; 5 success groups; 6 error groups | metadata, clean-file, stdin-check, multi-path, directory-check; errors: syntax-error, unused-import, undefined-name, missing-file, import-star, late-future |
| `checkstyle` | Java -> TypeScript | 4,692 | 457,537 | 23 cases; 12 success groups; 6 error groups | metadata, google-checks, output-format, tree-output, javadoc-output, suppression-output, exclude-filter, output-file, xpath-branch, debug-output, exclude-regex, tab-width; errors: parse-error, missing-file, missing-config, invalid-format, invalid-option-combination, invalid-suppression-position |
| `google-java-format` | Java -> Rust | 81 | 20,975 | 21 cases; 10 success groups; 5 error groups | metadata, format-output, stdin-format, style-option, replace-mode, dry-run, imports-mode, range-format, javadoc-mode, long-string-mode; errors: parse-error, missing-file, invalid-range, unknown-option, range-count-mismatch |

## Harness Coverage

The harness is still oracle-based: every case invokes the fixed official CLI and
then the agent-built executable with the same argv/stdin/files/environment.
The coverage groups above describe which major CLI behavior surfaces are
exercised by those oracle comparisons.

`eza` keeps the built-in `eza_cases()` matrix in the shared CLI-port runner.
The other nine tasks declare their cases and coverage groups in `tasks.json` so
the task-specific matrix is visible without opening the shared runner.

Coverage is intended to cover the main incompatible CLI surfaces for each tool:

- Search tools: pattern modes, file discovery, filtering, output modes, stdin,
  ignore/glob/type behavior, and representative errors.
- Formatter tools: parser/style modes, check/write/dry-run behavior, stdin,
  side effects, config-like controls, and parser/file/option errors.
- Compiler/analyzer tools: diagnostic modes, project/config inputs, output side
  effects, tree/metadata output, and invalid option/config/path errors.

## Report Code

All task-local report helpers are in `report/`:

- `report/generate-matrix-report.mjs` computes the matrix table from
  `tasks.json` and the local reference checkout cache.

Run it from the repository root:

```bash
node benchmark/tasks/refactoring/binary-verified-cli-port-matrix-10/report/generate-matrix-report.mjs
node benchmark/tasks/refactoring/binary-verified-cli-port-matrix-10/report/generate-matrix-report.mjs --json
```

The script only reports this task directory. It does not depend on broader
benchmark reporting code.
