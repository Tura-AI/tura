import test from "node:test";
import assert from "node:assert/strict";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
import { resolveGatewayUrl } from "../../../src/gateway/directory.js";

function withGatewayEnv<T>(value: string | undefined, run: () => T): T {
  const previous = process.env.TURA_BUILD_KIND;
  const previousHome = process.env.TURA_HOME;
  const previousUrl = process.env.TURA_GATEWAY_URL;
  const previousPort = process.env.TURA_GATEWAY_PORT;
  const home = mkdtempSync(join(tmpdir(), "tura-tui-gateway-url-"));
  if (value === undefined) delete process.env.TURA_BUILD_KIND;
  else process.env.TURA_BUILD_KIND = value;
  process.env.TURA_HOME = home;
  delete process.env.TURA_GATEWAY_URL;
  delete process.env.TURA_GATEWAY_PORT;
  try {
    return run();
  } finally {
    if (previous === undefined) delete process.env.TURA_BUILD_KIND;
    else process.env.TURA_BUILD_KIND = previous;
    if (previousHome === undefined) delete process.env.TURA_HOME;
    else process.env.TURA_HOME = previousHome;
    if (previousUrl === undefined) delete process.env.TURA_GATEWAY_URL;
    else process.env.TURA_GATEWAY_URL = previousUrl;
    if (previousPort === undefined) delete process.env.TURA_GATEWAY_PORT;
    else process.env.TURA_GATEWAY_PORT = previousPort;
    rmSync(home, { recursive: true, force: true });
  }
}

test("release TUI defaults to the release gateway port", () => {
  withGatewayEnv("release", () => {
    assert.equal(resolveGatewayUrl(), "http://127.0.0.1:4126");
  });
});

test("dev TUI defaults to the dev gateway port", () => {
  withGatewayEnv("dev", () => {
    assert.equal(resolveGatewayUrl(), "http://127.0.0.1:4125");
  });
});

test("active gateway record wins over build-kind defaults", () => {
  withGatewayEnv("release", () => {
    const activeDir = join(process.env.TURA_HOME as string, ".tura");
    mkdirSync(activeDir, { recursive: true });
    writeFileSync(
      join(activeDir, "gateway-active.env"),
      "TURA_GATEWAY_URL=http://127.0.0.1:4777\n",
    );

    assert.equal(resolveGatewayUrl(), "http://127.0.0.1:4777");
  });
});

test("explicit gateway URL still wins over build-kind defaults", () => {
  withGatewayEnv("release", () => {
    assert.equal(resolveGatewayUrl("http://127.0.0.1:4999"), "http://127.0.0.1:4999");
  });
});
