## Refactoring Operation Manual
Use this prompt when the work is to port, rebuild, refactor, or replace an existing repository, package, service, library, CLI, API, data processor, runtime, or application.

### Required workflow:
- Do not start by patching the new implementation. First learn the original implementation from its source code, official CLI/API surface, public contracts, tests, fixtures, schemas, examples, and live probes.
- Do not create standalone planning or documentation deliverables such as `architect.md`, behavior-table files, inventory files, compatibility reports, or other intermediate writeups unless the user explicitly asks for them.
- Build a differential verifier from the original implementation and the original public contract. The verifier must run the original implementation and the new implementation with the same generated inputs and compare the observable result.
- The source of truth is the original CLI/API/library behavior and the original contract implied by source, docs, tests, schemas, fixtures, and public interfaces. Do not invent a smaller substitute contract.
- Existing original-repository tests are mandatory validation. Port, adapt, or run all applicable original tests against the rebuilt implementation, and make them pass in the rebuilt codebase in addition to the generated verifier.

### Generator rule:
- Do not hard-code a small list of verifier inputs. For every public interface, command, API route, exported function, parser mode, config group, file/stdin/network/state input, and behavior group, define both:
  - a valid random generator that only produces inputs satisfying the original contract;
  - an invalid random generator that only produces inputs violating the original contract.
- Valid generators must prove the input is legal before using it. Invalid generators must prove the input is illegal before using it. Do not mix "probably valid" and "probably invalid" samples.
- Every verifier run must generate and pass at least 16 valid samples and 4 invalid samples for each non-trivial interface or behavior group. Regenerate samples on each run unless a deterministic seed is needed to reproduce a failure.
- For valid ranges, generate samples uniformly across the whole valid range, not only around local examples. Valid ranges include, but are not limited to, numeric ranges, indexes, time/depth/limit/precision/threshold values, row counts, column counts, field lengths, record sizes, page sizes, batch sizes, string lengths, collection sizes, nesting depths, path shapes, and similar measurable input domains.
- For strings and structured data, define the range from the input domain. Include, but do not limit generation to, empty/short/medium/long strings, whitespace, ASCII, Unicode, punctuation, quotes, delimiters, escapes, path-like text, numeric-looking text, case variants, repeated characters, embedded newlines, sparse/dense collections, duplicate keys, missing fields, extra fields, malformed fields, and type-mixed values when the interface accepts or rejects them.
- For tabular or CSV-like inputs, valid generators must produce contract-valid records and invalid generators must produce contract-invalid records. Cover, including but not limited to, row count, column count, header/no-header, delimiter kind, quote/escape form, empty fields, missing fields, extra fields, uneven rows, duplicate headers, sorted/unsorted values, null-like tokens, embedded newlines, trailing delimiters, leading/trailing spaces, and malformed records.
- For finite domains, generate every value when the domain is small. When the domain is large, generate representative classes plus rare, default, boundary, invalid, duplicate, and missing values.
- For parsers, formatters, algorithms, layout, ranking, serialization, math, search, or state machines, generate branch-boundary and whole-domain samples. A local fit that matches nearby examples but fails distant valid inputs is not compatibility.

### Oracle rule:
- Do not hard-code expected stdout, stderr, status, response body, return value, exception, mutation, state, or side effect.
- For every generated input, run the original implementation first and capture the oracle result live.
- Run the exact same generated input against the new implementation.
- Compare the observable result channels exposed by the original interface:
  - process/CLI: exit status, stdout, stderr;
  - API/service: response status, body, relevant headers, persisted state, and side effects;
  - library/module: return value, thrown exception type/message, mutation, observable side effects, and async result state.
- Choose the comparison shape required by the original contract: exact bytes/text, normalized exact, structured JSON/body equality, order-sensitive equality, semantic set equality, scalar equality, state equality, or error-channel equality. Do not make all comparisons loose, and do not make all comparisons exact when the contract is semantic.

### Implementation rule:
- Keep verifier data, generated cases, live oracle observations, and failure logs outside runtime implementation code.
- The new implementation must not read verifier artifacts, generated samples, oracle observations, fixture names, or failure logs to decide runtime output.
- Implement the real parser, API, module behavior, file/stdin/stdout/stderr/status, return value, exception, state, persistence, concurrency, and error behavior.
- Use verifier failures to identify the earliest contract boundary that differs from the original, fix that boundary, then rerun the verifier with fresh generated samples.

### Drift rule:
- Stop immediately if validation becomes help-only, metadata-only, invalid-only, happy-path-only, one-valid-case-only, or a fixed smoke list.
- Stop immediately if the verifier has hard-coded assertions for expected outputs instead of live oracle calls.
- Stop immediately if the verifier uses hand-picked inputs where a valid or invalid generator is possible.
- Stop immediately if valid and invalid generators do not both exist for each non-trivial interface or behavior group.
- Stop immediately if generated samples do not cover the full valid range, branch boundaries, and realistic fixture/state/input shapes from the original project.
- When drift happens, return to the original source/contract and repair the generators before patching more implementation code.

### Minimal verifier pattern:
Use code like this as a shape, adapted to the current repository. The verifier discovers the original contract, builds generators from that contract, creates fresh samples each run, and captures expected results live from the original implementation. It must not contain fixed input arrays or hand-written expected outputs.

```js
const assert = require("assert");

const rng = seededRng(process.env.VERIFIER_SEED || String(Date.now()));
const contract = discoverOriginalContract({
  sourceRoot: process.env.ORIGINAL_SOURCE_ROOT,
  docs: true,
  schemas: true,
  tests: true,
  fixtures: true,
  liveProbes: true,
});

function* validSamples(contract, rng) {
  for (const iface of contract.interfaces) {
    for (let i = 0; i < 16; i += 1) {
      // generateValid must sample across the whole valid domain for this
      // interface: ranges, strings, files, state, combinations, and boundaries.
      const sample = iface.generateValid(rng, i, 16);
      // The generator must prove validity from the original contract before the
      // sample is accepted. A sample that is merely "probably valid" is wrong.
      assert(iface.isValid(sample));
      yield { iface, sample };
    }
  }
}

function* invalidSamples(contract, rng) {
  for (const iface of contract.interfaces) {
    for (let i = 0; i < 4; i += 1) {
      // generateInvalid must intentionally violate the original contract:
      // malformed shape, invalid enum/range, missing required input, bad state,
      // invalid file/data format, or another proven illegal condition.
      const sample = iface.generateInvalid(rng, i, 4);
      // The generator must prove invalidity. Do not use ambiguous samples whose
      // legality depends on guessing instead of the original contract.
      assert(!iface.isValid(sample));
      yield { iface, sample };
    }
  }
}

for (const { iface, sample } of [...validSamples(contract, rng), ...invalidSamples(contract, rng)]) {
  // Expected behavior is captured live from the original implementation for
  // this exact generated input. Never write expected stdout/status/body/return
  // values into the verifier by hand.
  const oracle = iface.runOriginal(sample);
  const actual = iface.runPort(sample);
  // normalize must express the original contract: exact when exact is required,
  // semantic when the original contract is semantic, and error-channel aware for
  // invalid samples.
  assert.deepStrictEqual(iface.normalize(actual), iface.normalize(oracle));
}
```

### Validation:
- Run syntax checks, compile or wrapper checks, all applicable original-repository tests against the rebuilt implementation, the full generated differential verifier, and any available independent end-to-end validation before final response.
- Completion requires both gates to pass:
  - every applicable original-repository test case runs against the rebuilt implementation and passes;
  - every generated verifier run passes at least 16 valid samples and 4 invalid samples for each non-trivial interface or behavior group.
- Failed validation means unfinished work.
- Skipped validation means unfinished work.
- Timed-out validation means unfinished work until the timeout is understood or scoped.
- Weak, fixed, narrow, non-random, or output-hard-coded validation means unfinished work.
- Only claim completion when the full discovered behavior surface has generated valid/invalid oracle coverage, all applicable original tests pass on the rebuilt implementation, and the new implementation matches the original implementation on that coverage.
