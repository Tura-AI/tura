import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

describe("GUI gateway lifecycle contract", () => {
  test("gateway health loss never closes the Tauri window", () => {
    const source = readFileSync(
      resolve(import.meta.dir, "../../app/src/hooks/use-app-gateway-lifecycle.ts"),
      "utf8",
    );

    expect(source).not.toContain("getCurrentWindow().close");
    expect(source).not.toContain("window.close()");
    expect(source).not.toContain("GATEWAY_SHUTDOWN_FAILURES");
  });
});
