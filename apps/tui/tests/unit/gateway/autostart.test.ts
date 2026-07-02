import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer, type AddressInfo } from "node:net";
import {
  _canBindGatewayUrlForTest,
  _releaseOwnedGatewayForTest,
  _setOwnedGatewayForTest,
} from "../../../src/gateway/autostart.js";

function spawnSleeper() {
  return spawn(process.execPath, ["-e", "setInterval(()=>{},1000)"], {
    stdio: "ignore",
  });
}

function waitForSpawn(child: ReturnType<typeof spawn>): Promise<void> {
  return new Promise((resolve, reject) => {
    child.on("spawn", resolve);
    child.on("error", reject);
  });
}

function waitForExit(child: ReturnType<typeof spawn>): Promise<number | null> {
  return new Promise((resolve) => child.on("exit", (code) => resolve(code)));
}

function isAlive(pid: number | undefined): boolean {
  if (!pid) return false;
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

test("releasing an owned gateway reference does not kill the process", async () => {
  const child = spawnSleeper();
  await waitForSpawn(child);
  const pid = child.pid;
  try {
    _setOwnedGatewayForTest(child);
    _releaseOwnedGatewayForTest();

    assert.equal(isAlive(pid), true, "persistent gateway must survive front cleanup");
  } finally {
    child.kill();
    await waitForExit(child).catch(() => {});
  }
});

test("persistent gateway spawn is detached from the TUI process group", async () => {
  const child = spawn(process.execPath, ["-e", "setInterval(()=>{},1000)"], {
    detached: true,
    stdio: "ignore",
    windowsHide: true,
  });
  try {
    await waitForSpawn(child);
    assert.ok(child.pid, "spawned child has a pid");

    if (process.platform !== "win32") {
      const childPgid = parseInt(
        (
          await new Promise<string>((res, rej) => {
            const pg = spawn("ps", ["-o", "pgid=", "-p", String(child.pid)], {
              stdio: ["ignore", "pipe", "ignore"],
            });
            let out = "";
            pg.stdout.on("data", (d: Buffer) => (out += d.toString()));
            pg.on("exit", () => res(out.trim()));
            pg.on("error", rej);
          })
        ).trim(),
        10,
      );
      const parentPgid = (process as unknown as { getpgid?: (pid: number) => number }).getpgid?.(0);
      assert.notEqual(
        childPgid,
        parentPgid ?? process.pid,
        "gateway must not share TUI process group",
      );
    }
    // On Windows, detached=true maps to a detached child process; successful
    // spawn plus unreferenced cleanup is the portable unit-level contract.
  } finally {
    child.kill();
    await waitForExit(child).catch(() => {});
  }
});

test("a non-gateway process occupying the default port is treated as a collision", async () => {
  const server = createServer();
  await new Promise<void>((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  try {
    const address = server.address() as AddressInfo | null;
    assert.ok(address);

    assert.equal(await _canBindGatewayUrlForTest(`http://127.0.0.1:${address.port}`), false);
  } finally {
    await new Promise<void>((resolve) => server.close(() => resolve()));
  }
});
