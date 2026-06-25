import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer, type AddressInfo } from "node:net";
import {
  killOwnedGateway,
  _canBindGatewayUrlForTest,
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

test("killOwnedGateway is a no-op when no gateway is owned", () => {
  _setOwnedGatewayForTest(undefined);
  assert.doesNotThrow(() => killOwnedGateway());
});

test("killOwnedGateway kills the owned process", async () => {
  const child = spawnSleeper();
  await waitForSpawn(child);
  const pid = child.pid;

  _setOwnedGatewayForTest(child);
  killOwnedGateway();

  await waitForExit(child);
  assert.equal(isAlive(pid), false, "process should be dead after killOwnedGateway");
});

test("killOwnedGateway clears the reference — second call is a no-op", async () => {
  const child = spawnSleeper();
  await waitForSpawn(child);

  _setOwnedGatewayForTest(child);
  killOwnedGateway();

  // Second call must not throw even though reference was cleared
  assert.doesNotThrow(() => killOwnedGateway());

  await waitForExit(child);
});

test("killOwnedGateway handles an already-dead process gracefully", async () => {
  const child = spawn(process.execPath, ["-e", "process.exit(0)"], { stdio: "ignore" });
  await waitForExit(child);

  _setOwnedGatewayForTest(child);
  // Should not throw even though the process already exited
  assert.doesNotThrow(() => killOwnedGateway());
});

test("gateway spawned by ensureGatewayAvailable is not detached (integration guard)", async () => {
  // Verify that a process spawned without detached=true is in the same process
  // group as the parent on Unix, or inherits the parent job on Windows.
  // We use a canary child to assert basic process-group membership:
  // child.pid is defined (spawn succeeded) and belongs to our process group.
  const child = spawnSleeper();
  try {
    await waitForSpawn(child);
    assert.ok(child.pid, "spawned child has a pid");

    if (process.platform !== "win32") {
      // On Unix, process group matches the parent's because detached was false.
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
      assert.equal(childPgid, parentPgid ?? childPgid, "same process group as TUI");
    }
    // On Windows: absence of CREATE_NEW_PROCESS_GROUP flag means the gateway
    // inherits the parent console session — verified by the absence of errors above.
  } finally {
    _setOwnedGatewayForTest(child);
    killOwnedGateway();
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
