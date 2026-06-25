## Refactoring Operation Manual
Use this prompt when the work is to port, rebuild, refactor, or replace an existing CLI/API implementation.

### Required workflow:
- Do not start by patching the new implementation. First learn the original implementation.
- First create `architect.md`. Write the module design, data flow, command/API entry points, and compatibility risks.
- Then create a compatibility test framework. This framework must run the original implementation and the new implementation with the same inputs.
- Build a complete behavior table before claiming the work is done.
- The behavior table must include every command, subcommand, alias, mode, flag group, public API endpoint, input shape, output format, error case, exit/status code, stdout, stderr, fixture, stdin case, file case, and implementation file that matters.
- Use authoritative sources to build the table: `--help`, subcommand help, API docs, source signatures, command dispatchers, registries, tests, fixtures, and live original-implementation probes.
- If the source repo or official tests reveal 200 cases, build 200 matching oracle cases. Do not replace them with 20 or 30 hand-written smoke tests.
- If the original project has fixtures, stdin cases, directory cases, symlink cases, hidden-file cases, invalid cases, or format cases, your verifier must use the same kinds of cases.

### Oracle rule:
- Every verifier case must use one fixed input.
- Run that fixed input against the original CLI/API first.
- Save the original output as the oracle.
- For CLI work, save exit status, stdout, and stderr.
- For API work, save response status, response body, and relevant headers.
- Run the exact same fixed input against the new implementation.
- Assert that the new result equals the oracle.
- Do not write expected stdout, stderr, status, response body, or response status by hand.
- Do not guess expected values from memory, help text, metadata, or your own understanding.
- The expected value must come from the original implementation's live output for that same input.

### Implementation rule:
- Keep oracle data, behavior tables, verifier logs, validation reports, and official-reference observations outside runtime implementation code.
- The new implementation must not read verifier artifacts or oracle observations to decide its runtime output.
- Implement real parser, API, file, stdin, stdout, stderr, status, and error behavior.
- Use oracle failures to decide what to fix next.
- Keep fixing until every mapped valid case and invalid case matches the original implementation.

### Drift rule:
- Stop immediately if validation becomes help-only, metadata-only, invalid-only, or one-valid-case-only.
- Stop immediately if a green verifier covers only a small sample while the discovered surface is larger.
- Stop immediately if fixtures, stdin, env, working directory, or invocation distribution differ from the original tests or realistic source behavior.
- When drift happens, go back to the missing oracle-construction step before patching more implementation code.

### Simple probe examples:
Use small scripts like these to discover the behavior surface and to build live oracle checks. Adapt paths and commands to the current repository; do not hard-code expected output.

API docs version oracle probe:

```js
const assert = require("assert");

async function callApi(baseUrl, version, input) {
  const url = new URL(`/api/${version}/items/${input.id}`, baseUrl);
  url.searchParams.set("include", input.include);
  const response = await fetch(url);
  return {
    status: response.status,
    body: await response.text(),
  };
}

for (const version of discoverVersionsFromApiDocs()) {
  for (const fixedInput of fixtureInputsFor(version)) {
    const oracle = await callApi(process.env.OFFICIAL_API_BASE, version, fixedInput);
    const actual = await callApi(process.env.PORT_API_BASE, version, fixedInput);
    assert.deepStrictEqual(actual, oracle);
  }
}
```

CLI version differential probe:

```js
const assert = require("assert");
const { spawnSync } = require("child_process");

function run(bin, args) {
  const result = spawnSync(bin, args, { encoding: "utf8" });
  return {
    status: result.status,
    stdout: result.stdout,
    stderr: result.stderr,
  };
}

for (const fixedArgs of cliCasesFromHelpAndSource()) {
  const oracle = run(process.env.OFFICIAL_CLI, fixedArgs);
  const actual = run(process.env.PORT_CLI, fixedArgs);
  assert.deepStrictEqual(actual, oracle);
}
```

### Validation:
- Run syntax checks, compile or wrapper checks, the full oracle verifier, and any available independent end-to-end validation before final response.
- Failed validation means unfinished work.
- Skipped validation means unfinished work.
- Timed-out validation means unfinished work.
- Weak or narrow validation means unfinished work.
- Only claim completion when the full discovered CLI/API surface has executable oracle coverage and the new implementation matches the original implementation on that coverage.
