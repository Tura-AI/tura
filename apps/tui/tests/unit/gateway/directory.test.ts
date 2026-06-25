import test from "node:test";
import assert from "node:assert/strict";
import { resolveGatewayUrl } from "../../../src/gateway/directory.js";

function withBuildKind<T>(value: string | undefined, run: () => T): T {
  const previous = process.env.TURA_BUILD_KIND;
  if (value === undefined) delete process.env.TURA_BUILD_KIND;
  else process.env.TURA_BUILD_KIND = value;
  try {
    return run();
  } finally {
    if (previous === undefined) delete process.env.TURA_BUILD_KIND;
    else process.env.TURA_BUILD_KIND = previous;
  }
}

test("release TUI defaults to the release gateway port", () => {
  withBuildKind("release", () => {
    assert.equal(resolveGatewayUrl(), "http://127.0.0.1:4156");
  });
});

test("dev TUI defaults to the dev gateway port", () => {
  withBuildKind("dev", () => {
    assert.equal(resolveGatewayUrl(), "http://127.0.0.1:4126");
  });
});

test("explicit gateway URL still wins over build-kind defaults", () => {
  withBuildKind("release", () => {
    assert.equal(resolveGatewayUrl("http://127.0.0.1:4999"), "http://127.0.0.1:4999");
  });
});
