# tasks.json

Source: https://deepswe.datacurve.ai/artifacts/v1.1/tasks.json

{
  "scope": "Tasks attempted in this benchmark version",
  "n_tasks": 113,
  "rows": [
    {
      "base_commit_hash": "cb1b3b671d0ee9fa9da9f7b02f86967953ffd10a",
      "display_description": "Harden ABS module resolution and caching, expose cache introspection APIs, and make module flags work correctly in script mode.",
      "id": "abs-module-cache-flags",
      "language": "go",
      "problem_title": "Harden module loading, cache introspection, and script flags",
      "prompt_characters": 2524,
      "repository": "abs-lang/abs",
      "repository_url": "https://github.com/abs-lang/abs"
    },
    {
      "base_commit_hash": "cb1b3b671d0ee9fa9da9f7b02f86967953ffd10a",
      "display_description": "Add stepped slice parsing, slicing, and assignment for arrays and strings with rune-correct indexing.",
      "id": "abs-stepped-slices",
      "language": "go",
      "problem_title": "Add stepped slices for arrays and strings",
      "prompt_characters": 3384,
      "repository": "abs-lang/abs",
      "repository_url": "https://github.com/abs-lang/abs"
    },
    {
      "base_commit_hash": "0bdc95715fa58f64e3fd6e63b0f89be8733cbbab",
      "display_description": "Add a configurable lint rule that enforces pinned versions for action and reusable workflow references.",
      "id": "actionlint-action-pinning-lint",
      "language": "go",
      "problem_title": "Add action pinning linting for actions and reusable workflows",
      "prompt_characters": 2060,
      "repository": "rhysd/actionlint",
      "repository_url": "https://github.com/rhysd/actionlint"
    },
    {
      "base_commit_hash": "a691069fcadf9131e5f7a5a130a022dc678f3e1d",
      "display_description": "Add load-only alias support to name mapping so fields can resolve from alternate input keys.",
      "id": "adaptix-name-mapping-aliases",
      "language": "python",
      "problem_title": "Add input key aliases to name mapping",
      "prompt_characters": 982,
      "repository": "reagento/adaptix",
      "repository_url": "https://github.com/reagento/adaptix"
    },
    {
      "base_commit_hash": "b73fea2e0682803bda7531c93cd1dfb360839175",
      "display_description": "Add point-in-time task snapshots with interactive inspection, diffing, deletion, and web/CLI access.",
      "id": "aiomonitor-task-snapshots-diff",
      "language": "python",
      "problem_title": "Add task snapshots, inspection, and diffing to aiomonitor",
      "prompt_characters": 1733,
      "repository": "aio-libs/aiomonitor",
      "repository_url": "https://github.com/aio-libs/aiomonitor"
    },
    {
      "base_commit_hash": "9d2d84bb1564e9513287998c56ccf16c01c19008",
      "display_description": "Add parsing and call-time evaluation of default parameter values in Anko functions.",
      "id": "anko-default-function-arguments",
      "language": "go",
      "problem_title": "Add default arguments to Anko function parameters",
      "prompt_characters": 874,
      "repository": "mattn/anko",
      "repository_url": "https://github.com/mattn/anko"
    },
    {
      "base_commit_hash": "3f269a72ff69398b1250c584171f32d12c0d8085",
      "display_description": "Add typed `var` declarations in Anko and enforce declared type constraints on assignment when TypedBindings is enabled.",
      "id": "anko-typed-variable-bindings",
      "language": "go",
      "problem_title": "Add typed variable bindings to Anko",
      "prompt_characters": 1726,
      "repository": "mattn/anko",
      "repository_url": "https://github.com/mattn/anko"
    },
    {
      "base_commit_hash": "d34a5e2a6c5eb0f0955039775f5b9538424b58ff",
      "display_description": "Implement baseline capture, drift comparison, and compliance tracking for container configurations.",
      "id": "arcane-drift-detection-baselines",
      "language": "go",
      "problem_title": "Add drift detection and compliance baselines",
      "prompt_characters": 5385,
      "repository": "getarcaneapp/arcane",
      "repository_url": "https://github.com/getarcaneapp/arcane.git"
    },
    {
      "base_commit_hash": "04355e8b26d1ad5264ef62314a2bc46c4de58ed8",
      "display_description": "Add JSON Schema dependency keywords, local $defs/$ref resolution, and conditional schema handling.",
      "id": "arktype-json-schema-refs-dependencies",
      "language": "typescript",
      "problem_title": "Add JSON Schema refs and dependency keywords",
      "prompt_characters": 1893,
      "repository": "arktypeio/arktype",
      "repository_url": "https://github.com/arktypeio/arktype"
    },
    {
      "base_commit_hash": "82ac179c1de4c216c4e333093044fac643303f0c",
      "display_description": "Add dependency-aware asynchronous container initialization with ordered startup, concurrency limits, and rollback on failure.",
      "id": "awilix-async-container-initialization",
      "language": "typescript",
      "problem_title": "Add dependency-aware async initialization to the container",
      "prompt_characters": 2248,
      "repository": "jeffijoe/awilix",
      "repository_url": "https://github.com/jeffijoe/awilix"
    },
    {
      "base_commit_hash": "765f00d3f202f83f61d03f882f80a2d5142d81f8",
      "display_description": "Add incremental analysis caching with cache invalidation, import/export, pruning, and cache inspection CLI options.",
      "id": "bandit-incremental-cache-control",
      "language": "python",
      "problem_title": "Add incremental cache controls to Bandit",
      "prompt_characters": 1746,
      "repository": "PyCQA/bandit",
      "repository_url": "https://github.com/PyCQA/bandit.git"
    },
    {
      "base_commit_hash": "b46fa3a2723635aa29cc012538df4867ac2ac006",
      "display_description": "Track user input through variables and calls so Bandit flags injection sinks beyond string literals.",
      "id": "bandit-interprocedural-taint-checks",
      "language": "python",
      "problem_title": "Add interprocedural taint checks for Bandit injection sinks",
      "prompt_characters": 1081,
      "repository": "PyCQA/bandit",
      "repository_url": "https://github.com/PyCQA/bandit.git"
    },
    {
      "base_commit_hash": "b46fa3a2723635aa29cc012538df4867ac2ac006",
      "display_description": "Add region and next-line nosec directives with selector expressions and ignore-nosec handling.",
      "id": "bandit-structured-nosec-directives",
      "language": "python",
      "problem_title": "Add structured nosec directives for regions and next line",
      "prompt_characters": 2721,
      "repository": "PyCQA/bandit",
      "repository_url": "https://github.com/PyCQA/bandit.git"
    },
    {
      "base_commit_hash": "70409a5052984325dccfdc5f6520818568a81f39",
      "display_description": "Add cancellable evaluation handles that propagate through nested script, module, and job execution.",
      "id": "boa-hierarchical-evaluation-cancellation",
      "language": "rust",
      "problem_title": "Add hierarchical evaluation cancellation to Boa",
      "prompt_characters": 4154,
      "repository": "boa-dev/boa",
      "repository_url": "https://github.com/boa-dev/boa"
    },
    {
      "base_commit_hash": "6bc4708fb9b2ac52d9a18997e923da6a58916102",
      "display_description": "Add `partial_structure` and `PartialResult` for recoverable, field-level structuring with nested partial results.",
      "id": "cattrs-partial-structuring-recovery",
      "language": "python",
      "problem_title": "Add partial structuring with error recovery to cattrs",
      "prompt_characters": 1242,
      "repository": "python-attrs/cattrs",
      "repository_url": "https://github.com/python-attrs/cattrs"
    },
    {
      "base_commit_hash": "8a96e2dcd7f821d1250b58cf71c327679f94de25",
      "display_description": "Add async option fetching with caching, retries, debouncing, and loading state to AutocompletePrompt.",
      "id": "clack-async-autocomplete-options",
      "language": "typescript",
      "problem_title": "Add async autocomplete options and fetch lifecycle handling",
      "prompt_characters": 3029,
      "repository": "bombshell-dev/clack",
      "repository_url": "https://github.com/bombshell-dev/clack"
    },
    {
      "base_commit_hash": "5e0a2247d446c49a9951a06bb83b6e956dc7eb41",
      "display_description": "Run delegated sub-agents, feed their results back as tool_result, and handle unknown agents, failures, and circular delegation.",
      "id": "claude-code-by-agents-recursive-delegation",
      "language": "typescript",
      "problem_title": "Implement recursive agent delegation through delegate_task tool calls",
      "prompt_characters": 1199,
      "repository": "baryhuang/claude-code-by-agents",
      "repository_url": "https://github.com/baryhuang/claude-code-by-agents"
    },
    {
      "base_commit_hash": "132a437c40cffbdfbe474ca808c8debde59e2633",
      "display_description": "Add command-level config file loading, parsing, merging, and precedence handling.",
      "id": "cliffy-config-file-parsing",
      "language": "typescript",
      "problem_title": "Add config file parsing to Cliffy commands",
      "prompt_characters": 1973,
      "repository": "c4spar/cliffy",
      "repository_url": "https://github.com/c4spar/cliffy"
    },
    {
      "base_commit_hash": "88e3d965c0b1628642a30a841745b410d6835052",
      "display_description": "Add lexer methods to expand CSS shorthands into longhands and compress longhands back into shorthand values.",
      "id": "csstree-shorthand-expansion-compression",
      "language": "javascript",
      "problem_title": "Add shorthand expansion and compression to the lexer",
      "prompt_characters": 2799,
      "repository": "csstree/csstree",
      "repository_url": "https://github.com/csstree/csstree"
    },
    {
      "base_commit_hash": "0dd6132e0c58edbd9b1a5f7ffd00dfab1e6085ad",
      "display_description": "Add read and write support for HTML documents with Dasel's format handling.",
      "id": "dasel-html-document-format",
      "language": "go",
      "problem_title": "Add HTML document format handling to Dasel",
      "prompt_characters": 1546,
      "repository": "TomWright/dasel",
      "repository_url": "https://github.com/TomWright/dasel"
    },
    {
      "base_commit_hash": "c981f9c7aa91b83cc9bd33a09ecee9e751b06e8d",
      "display_description": "Extend rrule and rruleset to serialize, parse, and compare RFC 5545 timezone-aware recurrence data.",
      "id": "dateutil-rfc5545-timezone-interop",
      "language": "python",
      "problem_title": "Add RFC 5545 timezone interoperability to dateutil recurrence parsing",
      "prompt_characters": 2890,
      "repository": "dateutil/dateutil",
      "repository_url": "https://github.com/dateutil/dateutil"
    },
    {
      "base_commit_hash": "e8e6edfef5ca69c6188d320388ad440265911057",
      "display_description": "Add typed query-builder helpers for SQL window functions, named windows, and frame specs.",
      "id": "drizzle-orm-window-function-builders",
      "language": "typescript",
      "problem_title": "Add typed window function builders with OVER clauses",
      "prompt_characters": 2340,
      "repository": "drizzle-team/drizzle-orm",
      "repository_url": "https://github.com/drizzle-team/drizzle-orm"
    },
    {
      "base_commit_hash": "1f2a18664f8aded292707fcafb01ff15ea33d3b8",
      "display_description": "Add `requiredIf`-based conditional attribute enforcement across schema validation, parsing, updates, and JSON Schema export.",
      "id": "dynamodb-toolbox-conditional-attribute-requirements",
      "language": "typescript",
      "problem_title": "Add conditional required attributes to schemas",
      "prompt_characters": 1243,
      "repository": "dynamodb-toolbox/dynamodb-toolbox",
      "repository_url": "https://github.com/dynamodb-toolbox/dynamodb-toolbox"
    },
    {
      "base_commit_hash": "1f2a18664f8aded292707fcafb01ff15ea33d3b8",
      "display_description": "Add a lazy schema type for self-referencing recursive data with full serialization, validation, and export support.",
      "id": "dynamodb-toolbox-lazy-recursive-schemas",
      "language": "typescript",
      "problem_title": "Add lazy recursive schemas with DTO and JSON Schema export",
      "prompt_characters": 1265,
      "repository": "dynamodb-toolbox/dynamodb-toolbox",
      "repository_url": "https://github.com/dynamodb-toolbox/dynamodb-toolbox"
    },
    {
      "base_commit_hash": "9245bc59ebfa688e8c92dd691296ee69d0815e59",
      "display_description": "Add typed Server-Sent Events streaming endpoints, encoders/decoders, and client handling to HttpApi.",
      "id": "effect-sse-httpapi-streaming",
      "language": "typescript",
      "problem_title": "Add SSE streaming endpoints to HttpApi",
      "prompt_characters": 2177,
      "repository": "Effect-TS/effect",
      "repository_url": "https://github.com/Effect-TS/effect"
    },
    {
      "base_commit_hash": "68dafce",
      "display_description": "Add cursor-based keyset pagination to `$find` with nextCursor handling and validation.",
      "id": "eicrud-keyset-pagination-cursor",
      "language": "typescript",
      "problem_title": "Add keyset cursor pagination to `$find`",
      "prompt_characters": 1072,
      "repository": "eicrud/eicrud",
      "repository_url": "https://github.com/eicrud/eicrud"
    },
    {
      "base_commit_hash": "4032e04c8f2e2f35e43ce5d772fcef14a5df4d74",
      "display_description": "Add recursive XML diffing, patch generation and application, reverse patching, three-way merge, and diff summaries.",
      "id": "etree-xml-diff-patch",
      "language": "go",
      "problem_title": "Add XML diff, patch, and merge operations to etree",
      "prompt_characters": 4462,
      "repository": "beevik/etree",
      "repository_url": "https://github.com/beevik/etree"
    },
    {
      "base_commit_hash": "851b241a301f7c74646e65e4009c69cf290993a8",
      "display_description": "Add expression and block-level error recovery with try, catch, finally, throw, retry, and errtype.",
      "id": "expr-try-catch-errors",
      "language": "go",
      "problem_title": "Add try/catch error recovery to expr",
      "prompt_characters": 1356,
      "repository": "expr-lang/expr",
      "repository_url": "https://github.com/expr-lang/expr"
    },
    {
      "base_commit_hash": "11614be9021aa4ac078d4d0693a8b5250a1010d8",
      "display_description": "Add runtime Deprecation, Sunset, and Link headers plus OpenAPI metadata and tracking middleware for deprecated routes.",
      "id": "fastapi-deprecation-response-headers",
      "language": "python",
      "problem_title": "Add deprecation, sunset, and successor headers to FastAPI routes",
      "prompt_characters": 2972,
      "repository": "fastapi/fastapi",
      "repository_url": "https://github.com/fastapi/fastapi"
    },
    {
      "base_commit_hash": "11614be9021aa4ac078d4d0693a8b5250a1010d8",
      "display_description": "Add configurable implicit HEAD handling and automatic OPTIONS responses for FastAPI routes, routers, and included routers.",
      "id": "fastapi-implicit-head-options",
      "language": "python",
      "problem_title": "Add implicit HEAD and automatic OPTIONS responses to FastAPI routes",
      "prompt_characters": 1602,
      "repository": "fastapi/fastapi",
      "repository_url": "https://github.com/fastapi/fastapi"
    },
    {
      "base_commit_hash": "227883606023d62275fb48701aeac90f2b604143",
      "display_description": "Add repeatable multi-key sorting controls to fd output with deterministic tie-breaking and seeded random order.",
      "id": "fd-deterministic-multi-key-sorting",
      "language": "rust",
      "problem_title": "Add deterministic multi-key sorting to fd",
      "prompt_characters": 3483,
      "repository": "sharkdp/fd",
      "repository_url": "https://github.com/sharkdp/fd"
    },
    {
      "base_commit_hash": "87f5a40ea07a4ea629ee5623c72660f3d1b217fa",
      "display_description": "Add stable ShapeIndex Encode/Decode support so indices round-trip without rebuilding.",
      "id": "geo-shapeindex-serialization",
      "language": "go",
      "problem_title": "Add ShapeIndex encoding and decoding",
      "prompt_characters": 684,
      "repository": "golang/geo",
      "repository_url": "https://github.com/golang/geo"
    },
    {
      "base_commit_hash": "9aea378c4dccd6f4394196ad8f0873b3e84678c8",
      "display_description": "Add a diagnostic checker that validates doc comment symbol links against package and type information.",
      "id": "go-critic-doc-link-checker",
      "language": "go",
      "problem_title": "Add a checker for broken doc comment links",
      "prompt_characters": 1873,
      "repository": "go-critic/go-critic",
      "repository_url": "https://github.com/go-critic/go-critic"
    },
    {
      "base_commit_hash": "87c0e5a4f27d04569d927717769f34483e0ba475",
      "display_description": "Expose fully accumulated streamed function-call arguments across streaming, live sessions, and chat history.",
      "id": "go-genai-streamed-function-args",
      "language": "go",
      "problem_title": "Expose accumulated streamed function-call args in SDK surfaces",
      "prompt_characters": 1759,
      "repository": "googleapis/go-genai",
      "repository_url": "https://github.com/googleapis/go-genai.git"
    },
    {
      "base_commit_hash": "424e9964d3a33c6507a77c126841f2c5897262af",
      "display_description": "Add worktree merge support with conflict detection, merge head handling, and index stage updates.",
      "id": "go-git-worktree-merge-conflicts",
      "language": "go",
      "problem_title": "Add worktree merge conflict handling",
      "prompt_characters": 1825,
      "repository": "go-git/go-git",
      "repository_url": "https://github.com/go-git/go-git"
    },
    {
      "base_commit_hash": "399ef141161f212f4e81b5d7497b84633fc712d9",
      "display_description": "Implement per-artifact retries for uploads, artifactory, and blobs while recording deterministic publish attempt history.",
      "id": "goreleaser-retry-publish-auditing",
      "language": "go",
      "problem_title": "Add retry-aware publishing audit logs",
      "prompt_characters": 1698,
      "repository": "goreleaser/goreleaser",
      "repository_url": "https://github.com/goreleaser/goreleaser"
    },
    {
      "base_commit_hash": "f07c89f8f065010a36b4263eded209b2b1d37063",
      "display_description": "Add incremental GraphQL response handling with @defer and @stream across HTTP multipart, WebSocket transport, and the DSL.",
      "id": "gql-incremental-graphql-delivery",
      "language": "python",
      "problem_title": "Add GraphQL incremental delivery with @defer and @stream",
      "prompt_characters": 1458,
      "repository": "graphql-python/gql",
      "repository_url": "https://github.com/graphql-python/gql"
    },
    {
      "base_commit_hash": "82a0888cb2c87a6123e05424b528f8e8c9b3e426",
      "display_description": "Ensure interrupted request and response body reads, formData parsing, and discarded timers abort cleanly during shutdown.",
      "id": "happy-dom-abort-pending-body-reads",
      "language": "typescript",
      "problem_title": "Abort pending body reads on shutdown",
      "prompt_characters": 665,
      "repository": "capricorn86/happy-dom",
      "repository_url": "https://github.com/capricorn86/happy-dom"
    },
    {
      "base_commit_hash": "82a0888cb2c87a6123e05424b528f8e8c9b3e426",
      "display_description": "Implement a real, deterministic IntersectionObserver with async delivery, thresholds, root margins, and target tracking.",
      "id": "happy-dom-deterministic-intersectionobserver",
      "language": "typescript",
      "problem_title": "Implement a deterministic IntersectionObserver in Happy DOM",
      "prompt_characters": 1443,
      "repository": "capricorn86/happy-dom",
      "repository_url": "https://github.com/capricorn86/happy-dom"
    },
    {
      "base_commit_hash": "42f78ba60edf531d5161e00d9819a7c34d976343",
      "display_description": "Add chart-scoped append and key-based merge strategies for coalescing arrays during Helm value merging.",
      "id": "helm-array-merge-strategies",
      "language": "go",
      "problem_title": "Add configurable array merge strategies to Helm value coalescing",
      "prompt_characters": 2783,
      "repository": "helm/helm",
      "repository_url": "https://github.com/helm/helm"
    },
    {
      "base_commit_hash": "42f78ba60edf531d5161e00d9819a7c34d976343",
      "display_description": "Add a stable unified manifest stream for template, dry-run install/upgrade, and get manifest output.",
      "id": "helm-unified-manifest-stream",
      "language": "go",
      "problem_title": "Add unified manifest stream output across Helm commands",
      "prompt_characters": 954,
      "repository": "helm/helm",
      "repository_url": "https://github.com/helm/helm"
    },
    {
      "base_commit_hash": "b5addb64f0161ff6bfe94c124ef76f6a1fba5254",
      "display_description": "Add a deterministic CookieStore with modern Set-Cookie parsing, cookie eviction, and request header handling.",
      "id": "httpx-deterministic-cookie-store",
      "language": "typescript",
      "problem_title": "Add a deterministic CookieStore with modern Set-Cookie parsing",
      "prompt_characters": 3150,
      "repository": "encode/httpx",
      "repository_url": "https://github.com/encode/httpx"
    },
    {
      "base_commit_hash": "b5addb64f0161ff6bfe94c124ef76f6a1fba5254",
      "display_description": "Add Response iterators that parse multipart HTTP response bodies into parts.",
      "id": "httpx-multipart-response-parsing",
      "language": "python",
      "problem_title": "Add multipart response parsing to HTTPX",
      "prompt_characters": 2112,
      "repository": "encode/httpx",
      "repository_url": "https://github.com/encode/httpx"
    },
    {
      "base_commit_hash": "b5addb64f0161ff6bfe94c124ef76f6a1fba5254",
      "display_description": "Add response iterators that incrementally parse JSON values from supported streaming media types.",
      "id": "httpx-streaming-json-iteration",
      "language": "python",
      "problem_title": "Add streaming JSON iteration to HTTPX responses",
      "prompt_characters": 2532,
      "repository": "encode/httpx",
      "repository_url": "https://github.com/encode/httpx"
    },
    {
      "base_commit_hash": "bf4544d6c86ab4ace21254cb38a011ce3e845700",
      "display_description": "Persist and reuse the fitted feature schema so evaluation, prediction, serving, and export all apply the same canonicalized inputs.",
      "id": "igel-persist-feature-schema",
      "language": "python",
      "problem_title": "Persist the fitted feature schema across evaluate, predict, serve, and export",
      "prompt_characters": 1559,
      "repository": "nidhaloff/igel",
      "repository_url": "https://github.com/nidhaloff/igel"
    },
    {
      "base_commit_hash": "0cea59169ef0f3f83e4aa7fbedbff9d165646472",
      "display_description": "Add CSS Grid layout parsing and placement to the Box component, including track sizing, gaps, and explicit child positioning.",
      "id": "ink-grid-box-layout",
      "language": "typescript",
      "problem_title": "Add CSS Grid layout to the Box component",
      "prompt_characters": 841,
      "repository": "vadimdemedes/ink",
      "repository_url": "https://github.com/vadimdemedes/ink"
    },
    {
      "base_commit_hash": "0bb317d10fdcb3aa13beb1031d5f10e5b821203b",
      "display_description": "Add `.ipybundle` recording, validation, and replay APIs for IPython sessions.",
      "id": "ipython-session-bundle-replay",
      "language": "python",
      "problem_title": "Add session bundle recording and replay to IPython",
      "prompt_characters": 3590,
      "repository": "ipython/ipython",
      "repository_url": "https://github.com/ipython/ipython"
    },
    {
      "base_commit_hash": "89bede495dc2c85e1c57ba627a18526f71d57396",
      "display_description": "Add `\\multicolumn` parsing and rendering for array-like environments with span-aware alignment and errors.",
      "id": "katex-multicolumn-array-spans",
      "language": "javascript",
      "problem_title": "Add `\\multicolumn` column spans to array-like environments",
      "prompt_characters": 683,
      "repository": "KaTeX/KaTeX",
      "repository_url": "https://github.com/KaTeX/KaTeX"
    },
    {
      "base_commit_hash": "56b1fffecd743df1e7490235e69b51c44701f34c",
      "display_description": "Add a multiplexing layer that carries many ordered sub-streams over one KCP connection with flow control, priority scheduling, and SNMP counters.",
      "id": "kcp-go-multiplexed-kcp-streams",
      "language": "go",
      "problem_title": "Add multiplexed ordered streams over KCP",
      "prompt_characters": 2367,
      "repository": "xtaci/kcp-go",
      "repository_url": "https://github.com/xtaci/kcp-go"
    },
    {
      "base_commit_hash": "6c7ebba57821989733a11d6f3888816658584d97",
      "display_description": "Introduce fine-grained atomic selector tracking with dependency health, circular detection, and unchanged Kea lifecycle behavior.",
      "id": "kea-atomic-signal-selectors",
      "language": "typescript",
      "problem_title": "Add atomic signal selectors to Kea",
      "prompt_characters": 3376,
      "repository": "keajs/kea",
      "repository_url": "https://github.com/keajs/kea.git"
    },
    {
      "base_commit_hash": "7abc5278782e3280fec8292b39807ec1b537eaf4",
      "display_description": "Add spec.consistentHash to TrafficPolicy and merge it into route hash_policy generation.",
      "id": "kgateway-consistent-hash-policy",
      "language": "go",
      "problem_title": "Add consistent hash policy support to TrafficPolicy",
      "prompt_characters": 2181,
      "repository": "kgateway-dev/kgateway",
      "repository_url": "https://github.com/kgateway-dev/kgateway"
    },
    {
      "base_commit_hash": "3c5c1bd86376ee73d52a4cc770bdaeab15bbc2f3",
      "display_description": "Add single-active-consumer semantics, consumer priority selection, cancel notifications, and lifecycle tracking to virtual transports.",
      "id": "kombu-single-active-consumer-priority",
      "language": "python",
      "problem_title": "Add single-active-consumer priority and cancel tracking to virtual transports",
      "prompt_characters": 4265,
      "repository": "celery/kombu",
      "repository_url": "https://github.com/celery/kombu"
    },
    {
      "base_commit_hash": "3c5c1bd86376ee73d52a4cc770bdaeab15bbc2f3",
      "display_description": "Add dead-letter routing, TTL expiry, and max-length overflow handling to the virtual transport layer.",
      "id": "kombu-virtual-queue-dead-lettering",
      "language": "python",
      "problem_title": "Add dead-lettering, TTL, and overflow handling to virtual queues",
      "prompt_characters": 4383,
      "repository": "celery/kombu",
      "repository_url": "https://github.com/celery/kombu"
    },
    {
      "base_commit_hash": "9c434858b2b522002f8c5eb4a554fa8836a7cf3c",
      "display_description": "Add createAspect and aspect-aware query operations that merge constituent traits and propagate writes and lifecycle events.",
      "id": "koota-composite-trait-aspects",
      "language": "typescript",
      "problem_title": "Add composite trait aspects to Koota",
      "prompt_characters": 1508,
      "repository": "pmndrs/koota",
      "repository_url": "https://github.com/pmndrs/koota"
    },
    {
      "base_commit_hash": "31cbe9a1a26b3822a6c82ad50132508087cd24bc",
      "display_description": "Add a deferred command buffer that batches entity mutations during query iteration and flushes them at defined boundaries.",
      "id": "koota-deferred-mutation-buffer",
      "language": "typescript",
      "problem_title": "Add a deferred mutation buffer to batch entity changes",
      "prompt_characters": 932,
      "repository": "pmndrs/koota",
      "repository_url": "https://github.com/pmndrs/koota"
    },
    {
      "base_commit_hash": "72ebef44b8e024d877250f055eea60cdfaa4506",
      "display_description": "Add snapshot, rollback, and diff APIs for entities and worlds, including registry-based trait and relation serialization.",
      "id": "koota-entity-snapshot-rollback",
      "language": "python",
      "problem_title": "Add entity snapshot and rollback APIs to Koota",
      "prompt_characters": 2387,
      "repository": "pmndrs/koota",
      "repository_url": "https://github.com/pmndrs/koota"
    },
    {
      "base_commit_hash": "9c434858b2b522002f8c5eb4a554fa8836a7cf3c",
      "display_description": "Tracking modifiers should distinguish changes to specific relation pairs, not just trait-level additions and removals.",
      "id": "koota-pair-relation-tracking",
      "language": "typescript",
      "problem_title": "Add pair-level relation tracking modifiers",
      "prompt_characters": 981,
      "repository": "pmndrs/koota",
      "repository_url": "https://github.com/pmndrs/koota"
    },
    {
      "base_commit_hash": "9c434858b2b522002f8c5eb4a554fa8836a7cf3c",
      "display_description": "Add composable value-based entity predicates with dependency tracking and change transitions.",
      "id": "koota-query-predicates",
      "language": "typescript",
      "problem_title": "Add value-based query predicates to Koota",
      "prompt_characters": 859,
      "repository": "pmndrs/koota",
      "repository_url": "https://github.com/pmndrs/koota"
    },
    {
      "base_commit_hash": "91cf3733b2a419f5b17dff118cedb7052ab5300d",
      "display_description": "Add grouped aggregation clauses, window frame builders, null-handling modifiers, and related SQL function helpers.",
      "id": "kysely-window-grouping-helpers",
      "language": "typescript",
      "problem_title": "Add grouping-set and window-frame SQL helpers",
      "prompt_characters": 2633,
      "repository": "kysely-org/kysely",
      "repository_url": "https://github.com/kysely-org/kysely"
    },
    {
      "base_commit_hash": "7cef35b",
      "display_description": "Add `Runnable.with_coalesce()` so concurrent identical inputs share one execution across sync, async, streaming, and batch APIs.",
      "id": "langchain-request-coalescing",
      "language": "python",
      "problem_title": "Add request coalescing to `Runnable`",
      "prompt_characters": 1691,
      "repository": "langchain-ai/langchain",
      "repository_url": "https://github.com/langchain-ai/langchain"
    },
    {
      "base_commit_hash": "de139fd51c4d347666d109a8aea9d25451d908f6",
      "display_description": "Add field_options support for flattening nested dataclass fields into parent dictionaries with prefix and rename validation.",
      "id": "mashumaro-flattened-dataclass-fields",
      "language": "python",
      "problem_title": "Add flattened dataclass fields to Mashumaro field options",
      "prompt_characters": 471,
      "repository": "Fatal1ty/mashumaro",
      "repository_url": "https://github.com/Fatal1ty/mashumaro"
    },
    {
      "base_commit_hash": "d141eb14a40b79c04d1b1db5c20c6afa3844c0d9",
      "display_description": "Add parsing and AST support for `using` and `await using` declarations, including context-sensitive errors.",
      "id": "meriyah-explicit-resource-declarations",
      "language": "typescript",
      "problem_title": "Add explicit resource management declarations to the parser",
      "prompt_characters": 1256,
      "repository": "meriyah/meriyah",
      "repository_url": "https://github.com/meriyah/meriyah"
    },
    {
      "base_commit_hash": "73f5b537c8cad998e8e6d6bc40ad60e2e23bf268",
      "display_description": "Add daemon watch validation, state tracking, logging, and lifecycle commands with non-blocking run-once processing.",
      "id": "mnamer-daemon-watch-lifecycle",
      "language": "python",
      "problem_title": "Validate daemon watch, status, and log lifecycle",
      "prompt_characters": 2839,
      "repository": "jkwill87/mnamer",
      "repository_url": "https://github.com/jkwill87/mnamer"
    },
    {
      "base_commit_hash": "ec052921917ef201e73cc8e275dc91c5706b345f",
      "display_description": "Add grouped execution phases with explicit synchronization barriers and per-group setup and teardown.",
      "id": "mobly-grouped-test-barriers",
      "language": "python",
      "problem_title": "Add grouped test phases with synchronized barriers",
      "prompt_characters": 2518,
      "repository": "google/mobly",
      "repository_url": "https://github.com/google/mobly"
    },
    {
      "base_commit_hash": "061c97f8a01bf9e721835978b039303c5051501c",
      "display_description": "Add the remaining rolling window methods to Expr and Series with consistent validation and backend delegation.",
      "id": "narwhals-rolling-window-suite",
      "language": "python",
      "problem_title": "Add rolling min, max, median, and quantile methods",
      "prompt_characters": 2140,
      "repository": "narwhals-dev/narwhals",
      "repository_url": "https://github.com/narwhals-dev/narwhals"
    },
    {
      "base_commit_hash": "5781334aa654972fdc749003e7c1e93e6d277110",
      "display_description": "Add configurable boundary handling modes to `@stencil` for out-of-bounds accesses.",
      "id": "numba-stencil-boundary-modes",
      "language": "python",
      "problem_title": "Add boundary modes to `@stencil`",
      "prompt_characters": 797,
      "repository": "numba/numba",
      "repository_url": "https://github.com/numba/numba"
    },
    {
      "base_commit_hash": "6393b3ab32a2ace1fc24d4b0f5e0f13a179c874f",
      "display_description": "Implement an opt-in rule that generates and updates a TOC from document headings.",
      "id": "obsidian-linter-auto-table-of-contents",
      "language": "typescript",
      "problem_title": "Add automatic table of contents generation for Obsidian linter",
      "prompt_characters": 1459,
      "repository": "platers/obsidian-linter",
      "repository_url": "https://github.com/platers/obsidian-linter"
    },
    {
      "base_commit_hash": "6393b3ab32a2ace1fc24d4b0f5e0f13a179c874f",
      "display_description": "Add a Link Style rule that converts between Obsidian wiki links or embeds and markdown links or images while preserving ignored regions and edge cases.",
      "id": "obsidian-linter-link-format-conversion",
      "language": "typescript",
      "problem_title": "Add link format conversion between wiki and markdown syntax",
      "prompt_characters": 2097,
      "repository": "platers/obsidian-linter",
      "repository_url": "https://github.com/platers/obsidian-linter"
    },
    {
      "base_commit_hash": "6393b3ab32a2ace1fc24d4b0f5e0f13a179c874f",
      "display_description": "Implement standalone comment markers that disable and re-enable specific lint rules with nested scope handling.",
      "id": "obsidian-linter-scoped-ignore-markers",
      "language": "typescript",
      "problem_title": "Add scoped per-rule ignore markers to Obsidian Linter",
      "prompt_characters": 2142,
      "repository": "platers/obsidian-linter",
      "repository_url": "https://github.com/platers/obsidian-linter"
    },
    {
      "base_commit_hash": "dfbe3ca4ef8a22fc023fca5a5ef530e525f5e523",
      "display_description": "Add an opt-in per-origin circuit breaker for fetch requests with half-open probing and shared state across clients.",
      "id": "ofetch-per-origin-circuit-breaker",
      "language": "typescript",
      "problem_title": "Add a per-origin circuit breaker to ofetch",
      "prompt_characters": 3376,
      "repository": "unjs/ofetch",
      "repository_url": "https://github.com/unjs/ofetch"
    },
    {
      "base_commit_hash": "a48e806195c538a73b6916b281939577b370952d",
      "display_description": "Encrypt database dumps before upload and add keyed config, loading, and decryptable streaming support.",
      "id": "onedump-dump-encryption-pipeline",
      "language": "go",
      "problem_title": "Add transparent encryption to dump uploads",
      "prompt_characters": 2449,
      "repository": "liweiyi88/onedump",
      "repository_url": "https://github.com/liweiyi88/onedump"
    },
    {
      "base_commit_hash": "1ac64ef1a57a531c2723c59848890b88e816d777",
      "display_description": "Add opt-in per-rule evaluation profiling to Rego results with profiling stats and diff helpers.",
      "id": "opa-rego-rule-profiling",
      "language": "go",
      "problem_title": "Add rule evaluation profiling to Rego",
      "prompt_characters": 2849,
      "repository": "open-policy-agent/opa",
      "repository_url": "https://github.com/open-policy-agent/opa"
    },
    {
      "base_commit_hash": "1ac64ef1a57a531c2723c59848890b88e816d777",
      "display_description": "Rebuild lowered template strings into normal Rego syntax in partial evaluation outputs and support modules.",
      "id": "opa-template-string-reconstruction",
      "language": "go",
      "problem_title": "Reconstruct template strings in partial evaluation output",
      "prompt_characters": 1042,
      "repository": "open-policy-agent/opa",
      "repository_url": "https://github.com/open-policy-agent/opa"
    },
    {
      "base_commit_hash": "14bbe4efc7ded67932771b9ca18d9d637bb4cf27",
      "display_description": "Add conditional dependencies so options can require or hide based on other option presence or values.",
      "id": "optique-conditional-option-dependencies",
      "language": "typescript",
      "problem_title": "Add conditional option dependencies to Optique",
      "prompt_characters": 2991,
      "repository": "dahlia/optique",
      "repository_url": "https://github.com/dahlia/optique"
    },
    {
      "base_commit_hash": "1fd7fab851ecc975e008be0e3e279568ce4e2b51",
      "display_description": "Keep rewrite optimizations from breaking structure-dependent stylesheet selector matching.",
      "id": "oxvg-structural-selector-preservation",
      "language": "rust",
      "problem_title": "Preserve structure needed by stylesheet selectors",
      "prompt_characters": 757,
      "repository": "noahbald/oxvg",
      "repository_url": "https://github.com/noahbald/oxvg"
    },
    {
      "base_commit_hash": "1051d4767b5a469936daf5f1cebb63da6c9fb776",
      "display_description": "Add build-time static analysis that detects ambiguous participle grammars and reports conflicts.",
      "id": "participle-grammar-conflict-analysis",
      "language": "go",
      "problem_title": "Add build-time grammar conflict analysis to participle",
      "prompt_characters": 2991,
      "repository": "alecthomas/participle",
      "repository_url": "https://github.com/alecthomas/participle.git"
    },
    {
      "base_commit_hash": "1454d2bc0f378d7f34766afafee68a77e7b85995",
      "display_description": "Add batch durability callbacks, durability wait methods, notifications, and metrics for sync write commits.",
      "id": "pebble-durability-wait-apis",
      "language": "go",
      "problem_title": "Add durability callbacks and wait APIs for sync writes",
      "prompt_characters": 2660,
      "repository": "cockroachdb/pebble",
      "repository_url": "https://github.com/cockroachdb/pebble"
    },
    {
      "base_commit_hash": "79dd30d11aab6f0fba3cd79bd48f456209b966b3",
      "display_description": "Add optimizer passes that collapse qualifying choice chains into merged character and negated character classes.",
      "id": "pest-character-class-coalescing",
      "language": "rust",
      "problem_title": "Coalesce qualifying choices into character classes",
      "prompt_characters": 1146,
      "repository": "pest-parser/pest",
      "repository_url": "https://github.com/pest-parser/pest"
    },
    {
      "base_commit_hash": "24a057bbf9089677b4c49eac4ae1f28287ac8bb9",
      "display_description": "Add an opt-in transactional config reload mode with durable reload outcomes, rollback tracking, and an HTTP status endpoint.",
      "id": "prometheus-transactional-reload-status",
      "language": "typescript",
      "problem_title": "Add transactional reload status and rollback tracking to Prometheus",
      "prompt_characters": 1841,
      "repository": "prometheus/prometheus",
      "repository_url": "https://github.com/prometheus/prometheus.git"
    },
    {
      "base_commit_hash": "8b25b26a7653d9c7444f217a7f2ae9b327bda921",
      "display_description": "PromQL label sorting must order mixed typed and untyped label values with stable typed comparison rules.",
      "id": "prometheus-typed-label-sorting",
      "language": "go",
      "problem_title": "Fix PromQL label sorting across typed and untyped values",
      "prompt_characters": 1529,
      "repository": "prometheus/prometheus",
      "repository_url": "https://github.com/prometheus/prometheus"
    },
    {
      "base_commit_hash": "c5e03189188daa3c5589326a9d74506d7dc48bc9",
      "display_description": "Add typed blend range objects, persist them on layers, and apply blend-if during compositing.",
      "id": "psd-tools-blend-range-api",
      "language": "python",
      "problem_title": "Add typed blend range access and blend-if compositing",
      "prompt_characters": 2448,
      "repository": "psd-tools/psd-tools",
      "repository_url": "https://github.com/psd-tools/psd-tools"
    },
    {
      "base_commit_hash": "76894a5404a65d2800b6d0adaf3485ecba275caa",
      "display_description": "Add a TubeMultiplexer and MuxChannel API for multiple bidirectional logical channels over one tube, with flow control and close propagation.",
      "id": "pwntools-tube-multiplexing",
      "language": "python",
      "problem_title": "Add tube multiplexing to pwntools",
      "prompt_characters": 3450,
      "repository": "Gallopsled/pwntools",
      "repository_url": "https://github.com/Gallopsled/pwntools"
    },
    {
      "base_commit_hash": "8d17ba9f6ba8420cf05fddb94013bc221ed9a222",
      "display_description": "Add per-state scoped data ownership with lifecycle resets, callback injection, history restoration, and validation.",
      "id": "python-statemachine-state-data-scoping",
      "language": "python",
      "problem_title": "Add scoped state data to state machine callbacks and history",
      "prompt_characters": 1857,
      "repository": "fgmacedo/python-statemachine",
      "repository_url": "https://github.com/fgmacedo/python-statemachine"
    },
    {
      "base_commit_hash": "1047cdc393fac7c98822c993d70c28f58833c63d",
      "display_description": "Preserve full persisted query state, including errors, counters, timestamps, and infinite pagination, during restoration and cache rebuilds.",
      "id": "query-persist-restored-query-state",
      "language": "typescript",
      "problem_title": "Preserve restored query state in persisted snapshots",
      "prompt_characters": 2835,
      "repository": "TanStack/query",
      "repository_url": "https://github.com/TanStack/query"
    },
    {
      "base_commit_hash": "539cbffd0a13b18e9c65eb84dd35e6596e403158",
      "display_description": "Allow multiple Quill editors to share one toolbar container while routing toolbar actions and UI state to the currently active editor.",
      "id": "quill-shared-toolbar-focus",
      "language": "typescript",
      "problem_title": "Reuse one toolbar across multiple Quill editors",
      "prompt_characters": 1510,
      "repository": "slab/quill",
      "repository_url": "https://github.com/slab/quill"
    },
    {
      "base_commit_hash": "41607fae1289de2787523c452d75212206b9c7c0",
      "display_description": "Add a Validated container that accumulates errors while preserving standard container APIs.",
      "id": "returns-validated-error-accumulation",
      "language": "python",
      "problem_title": "Add an error-accumulating Validated container",
      "prompt_characters": 3137,
      "repository": "dry-python/returns",
      "repository_url": "https://github.com/dry-python/returns"
    },
    {
      "base_commit_hash": "bc2796e01998ebc2d40818323f93113aed2542ea",
      "display_description": "Add an opt-in bounded-memory mode that spills per-file aggregation results to disk without changing output.",
      "id": "scc-bounded-memory-spilling",
      "language": "go",
      "problem_title": "Add bounded-memory spilling to SCC aggregation",
      "prompt_characters": 1916,
      "repository": "boyter/scc",
      "repository_url": "https://github.com/boyter/scc.git"
    },
    {
      "base_commit_hash": "11703bb5e02cca28d08fe83ac9a4bdd2e087235e",
      "display_description": "Implement method declarations with receiver handling, method expressions, and interface satisfaction for Scriggo-defined types.",
      "id": "scriggo-method-declarations",
      "language": "go",
      "problem_title": "Add method declarations and interface dispatch to Scriggo",
      "prompt_characters": 958,
      "repository": "open2b/scriggo",
      "repository_url": "https://github.com/open2b/scriggo"
    },
    {
      "base_commit_hash": "24c4466fea94f551fb73d21eba54038dc5b346d3",
      "display_description": "Add a DurationEncoder, duration selector, and duration routing in TableVectorizer.",
      "id": "skrub-duration-encoding",
      "language": "python",
      "problem_title": "Add duration encoding to TableVectorizer",
      "prompt_characters": 3348,
      "repository": "skrub-data/skrub",
      "repository_url": "https://github.com/skrub-data/skrub"
    },
    {
      "base_commit_hash": "954e5a474b9e3d45ca58f02a3a4eac8e1947acc5",
      "display_description": "Add parsing and formatting support for BigQuery pipe syntax queries without changing traditional SQL formatting.",
      "id": "sql-formatter-bigquery-pipe-formatting",
      "language": "typescript",
      "problem_title": "Format BigQuery pipe syntax queries correctly",
      "prompt_characters": 1697,
      "repository": "sql-formatter-org/sql-formatter",
      "repository_url": "https://github.com/sql-formatter-org/sql-formatter"
    },
    {
      "base_commit_hash": "da140993a4547170ef85dc5ce7ce1c270f4322b3",
      "display_description": "Format CREATE TABLE statements with DDL-aware line breaking and add parsing models for table columns and constraints.",
      "id": "sqlfmt-create-table-ddl-formatting",
      "language": "python",
      "problem_title": "Format CREATE TABLE DDL and add DDL parsing helpers",
      "prompt_characters": 2969,
      "repository": "tconbeer/sqlfmt",
      "repository_url": "https://github.com/tconbeer/sqlfmt"
    },
    {
      "base_commit_hash": "8d74ffc93292c604d5827e2b44fffedca0c28c19",
      "display_description": "Add safe bulk import checkpoints, invariant validation, and rollback-on-failure behavior.",
      "id": "sqlite-utils-safe-import-checkpoints",
      "language": "python",
      "problem_title": "Add safe import checkpoints and invariant validation",
      "prompt_characters": 2509,
      "repository": "simonw/sqlite-utils",
      "repository_url": "https://github.com/simonw/sqlite-utils"
    },
    {
      "base_commit_hash": "010c4bdb4b8758844fd44eacf38e42b22eba8aea",
      "display_description": "Add configurable serialization and restoration of error stacks, stack frames, causes, and sanitization in SuperJSON.",
      "id": "superjson-error-stack-serialization",
      "language": "typescript",
      "problem_title": "Add error stack serialization to SuperJSON",
      "prompt_characters": 4061,
      "repository": "flightcontrolhq/superjson",
      "repository_url": "https://github.com/flightcontrolhq/superjson.git"
    },
    {
      "base_commit_hash": "54bdcba369357b47e19066b57badfb216a4c8d95",
      "display_description": "Add a graph export command for tasks with JSON, DOT, and text output, including reverse traversal and status control.",
      "id": "task-task-graph-export",
      "language": "go",
      "problem_title": "Add task graph export with JSON, DOT, and text output",
      "prompt_characters": 2510,
      "repository": "go-task/task",
      "repository_url": "https://github.com/go-task/task"
    },
    {
      "base_commit_hash": "3cad0da7a51b1206c6f01e3f4fbb44b976d5275c",
      "display_description": "Enable Go-side invocation of exported Tengo functions and closures while preserving runtime context and isolating compiled-instance state.",
      "id": "tengo-callable-instance-isolation",
      "language": "go",
      "problem_title": "Fix isolated Go-side calls for Tengo callables and closures",
      "prompt_characters": 1246,
      "repository": "d5/tengo",
      "repository_url": "https://github.com/d5/tengo"
    },
    {
      "base_commit_hash": "3cad0da7a51b1206c6f01e3f4fbb44b976d5275c",
      "display_description": "Add destructuring bindings for `:=` in arrays, maps, and function parameters.",
      "id": "tengo-destructuring-bindings",
      "language": "go",
      "problem_title": "Add destructuring bindings to Tengo",
      "prompt_characters": 952,
      "repository": "d5/tengo",
      "repository_url": "https://github.com/d5/tengo"
    },
    {
      "base_commit_hash": "368a3572b8146cc038b3f240da6792003d7e42c5",
      "display_description": "Add ANSI tokenization, reset-preserving truncation, and style-aware output helpers.",
      "id": "termenv-preserve-ansi-resets",
      "language": "go",
      "problem_title": "Preserve ANSI resets during truncation and styling",
      "prompt_characters": 1557,
      "repository": "muesli/termenv",
      "repository_url": "https://github.com/muesli/termenv"
    },
    {
      "base_commit_hash": "06a1adb7a70e85e7322d8cfae3181508785de95d",
      "display_description": "Add configurable early bailout on test failures across runners, reporters, and exit codes.",
      "id": "testem-bail-on-test-failure",
      "language": "javascript",
      "problem_title": "Add bail-on-test-failure handling to Testem",
      "prompt_characters": 2376,
      "repository": "testem/testem",
      "repository_url": "https://github.com/testem/testem"
    },
    {
      "base_commit_hash": "158f61ea91c9613d2011c41ee9be40ada1d7a307",
      "display_description": "Split report output into per-launcher files with template expansion, launcher-safe filenames, and per-launcher summaries.",
      "id": "testem-per-launcher-reports",
      "language": "javascript",
      "problem_title": "Partition report files by launcher and expand report templates",
      "prompt_characters": 2092,
      "repository": "testem/testem",
      "repository_url": "https://github.com/testem/testem"
    },
    {
      "base_commit_hash": "9737a5ab723f79e59f0a83eb036a3d15fad6b054",
      "display_description": "Add full Kitty keyboard phase handling and stable fallback key metadata for keys and shortcuts.",
      "id": "textual-kitty-key-phases",
      "language": "python",
      "problem_title": "Complete Kitty keyboard phases and stable fallback key metadata",
      "prompt_characters": 1665,
      "repository": "Textualize/textual",
      "repository_url": "https://github.com/Textualize/textual"
    },
    {
      "base_commit_hash": "0f0849fd37fbd0d4d6f81889476c22340129df67",
      "display_description": "Restore RichLog follow-mode parity with Log and preserve expand reflow behavior for justified writes.",
      "id": "textual-richlog-follow-state",
      "language": "python",
      "problem_title": "Restore RichLog follow-state parity and expand reflow behavior",
      "prompt_characters": 1564,
      "repository": "Textualize/textual",
      "repository_url": "https://github.com/Textualize/textual"
    },
    {
      "base_commit_hash": "dd05eebc8ed9e30fc6c223088a5a450cb54c1cab",
      "display_description": "Add in-place conversion helpers between standard tables, inline tables, dotted keys, and super tables while preserving comments and round-trip integrity.",
      "id": "tomlkit-toml-table-converters",
      "language": "python",
      "problem_title": "Add bidirectional TOML table converters",
      "prompt_characters": 1944,
      "repository": "python-poetry/tomlkit",
      "repository_url": "https://github.com/python-poetry/tomlkit"
    },
    {
      "base_commit_hash": "d8fbebc75de4991a32354518beff1abf628d0b07",
      "display_description": "Add iterable-aware sequence, traverse, zip, filtering, and task combinators across Maybe, Result, Task, and toolbelt APIs.",
      "id": "true-myth-iterable-collection-combinators",
      "language": "typescript",
      "problem_title": "Add iterable collection combinators to true-myth",
      "prompt_characters": 1975,
      "repository": "true-myth/true-myth",
      "repository_url": "https://github.com/true-myth/true-myth"
    },
    {
      "base_commit_hash": "f66fc061fde4f764b113ededa09be63dae564159",
      "display_description": "Add a new `matchEach` matcher that evaluates all matching clauses and returns all results in order.",
      "id": "ts-pattern-match-each",
      "language": "typescript",
      "problem_title": "Add `matchEach` to ts-pattern",
      "prompt_characters": 2970,
      "repository": "gvergnaud/ts-pattern",
      "repository_url": "https://github.com/gvergnaud/ts-pattern"
    },
    {
      "base_commit_hash": "9ecd74f5bd56fa915501e5b77da044d97c450a74",
      "display_description": "Add policy-driven alerting with consecutive failure, latency, recovery, and SSL expiry notifications.",
      "id": "updo-policy-alerting",
      "language": "go",
      "problem_title": "Add policy-based alerting for failures, latency, and SSL expiry",
      "prompt_characters": 4471,
      "repository": "Owloops/updo",
      "repository_url": "https://github.com/Owloops/updo"
    },
    {
      "base_commit_hash": "50016c77c808f9ca80391cf1abc96cc5416cf57d",
      "display_description": "Add first-class recursive schema composition with `Recur`, `recursive(...)`, and `recursiveAsync(...)` wrappers.",
      "id": "valibot-recursive-schema-composition",
      "language": "typescript",
      "problem_title": "Add recursive schema composition to Valibot",
      "prompt_characters": 1552,
      "repository": "open-circle/valibot",
      "repository_url": "https://github.com/open-circle/valibot"
    },
    {
      "base_commit_hash": "647e6ade3b99523e3a0387a65fccfe918c331236",
      "display_description": "Add duration-aware sharding strategies and duration history handling for Vitest.",
      "id": "vitest-duration-sharding",
      "language": "typescript",
      "problem_title": "Add duration-aware sharding to Vitest",
      "prompt_characters": 4311,
      "repository": "vitest-dev/vitest",
      "repository_url": "https://github.com/vitest-dev/vitest.git"
    },
    {
      "base_commit_hash": "1eb212f0a0707ad6f4c720bb2010c2b7517cf0f9",
      "display_description": "Add a persistent cache so Vulture can reuse unchanged analysis across runs.",
      "id": "vulture-persistent-analysis-cache",
      "language": "python",
      "problem_title": "Add a persistent analysis cache to Vulture",
      "prompt_characters": 2280,
      "repository": "jendrikseipp/vulture",
      "repository_url": "https://github.com/jendrikseipp/vulture"
    },
    {
      "base_commit_hash": "e1f76e285b9ad68a952b7cf5297bbb7ab91e6028",
      "display_description": "Generate opt-in Wasm coredumps on traps and attach the bytes to errors.",
      "id": "wasmi-trap-coredumps",
      "language": "rust",
      "problem_title": "Add trap coredump generation to wasmi",
      "prompt_characters": 3058,
      "repository": "wasmi-labs/wasmi",
      "repository_url": "https://github.com/wasmi-labs/wasmi"
    },
    {
      "base_commit_hash": "3ec1e028c8cbda984a71bf72321008723ebdcb51",
      "display_description": "Add coordinated capture, restore, diff, and serialization for multi-module memory snapshots.",
      "id": "wazero-multi-module-snapshots",
      "language": "go",
      "problem_title": "Add multi-module memory snapshots to wazero",
      "prompt_characters": 3584,
      "repository": "wazero/wazero",
      "repository_url": "https://github.com/wazero/wazero.git"
    },
    {
      "base_commit_hash": "fcb76d1ece0c3edc2548c39aa5b170475d2261bb",
      "display_description": "Support //go:embed directives so interpreted package variables can receive embedded file contents and embed.FS values.",
      "id": "yaegi-go-embed-directives",
      "language": "go",
      "problem_title": "Add go:embed directive support for interpreted packages",
      "prompt_characters": 1300,
      "repository": "traefik/yaegi",
      "repository_url": "https://github.com/traefik/yaegi"
    },
    {
      "base_commit_hash": "7795050a749bd1111cbbdd9d0219b27226a8e710",
      "display_description": "Add strict, deterministic conflict detection for Y.Map key writes with collect and error policies.",
      "id": "yjs-map-conflict-detection",
      "language": "javascript",
      "problem_title": "Add deterministic map conflict detection to Y.Map writes",
      "prompt_characters": 1659,
      "repository": "yjs/yjs",
      "repository_url": "https://github.com/yjs/yjs"
    },
    {
      "base_commit_hash": "452382821dd9dae7cc36995960656bb94dc47212",
      "display_description": "Add orderedmap and Starlark JSONPath query APIs with selectors, filters, and syntax errors.",
      "id": "ytt-jsonpath-query-api",
      "language": "go",
      "problem_title": "Add JSONPath query APIs to orderedmap and Starlark modules",
      "prompt_characters": 2418,
      "repository": "carvel-dev/ytt",
      "repository_url": "https://github.com/carvel-dev/ytt"
    }
  ]
}
