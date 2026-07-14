import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { join } from "node:path";
import { tmpdir } from "node:os";
import test from "node:test";
import {
  mediaTokenForInputPath,
  saveClipboardImageInput,
  saveInputBytes,
} from "../../../src/tui/clipboard-image.js";

test("saveInputBytes writes media input files under the workspace", async () => {
  const workspace = await mkdtemp(join(tmpdir(), "tura-tui-input-"));
  try {
    const relative = await saveInputBytes(workspace, "../screen shot.png", Buffer.from([1, 2, 3]));

    assert.match(relative, /^\.tura\/media\/input\//u);
    assert.ok(relative.endsWith("screen-shot.png"));
    assert.deepEqual(await readFile(join(workspace, relative)), Buffer.from([1, 2, 3]));
    assert.equal(mediaTokenForInputPath(relative), `[MEDIA:${relative}:MEDIA]`);
  } finally {
    await rm(workspace, { recursive: true, force: true });
  }
});

test("saveClipboardImageInput reads the test clipboard image and saves a workspace token target", async () => {
  const workspace = await mkdtemp(join(tmpdir(), "tura-tui-clipboard-"));
  const previousImage = process.env.TURA_TUI_CLIPBOARD_IMAGE_BASE64;
  const previousName = process.env.TURA_TUI_CLIPBOARD_IMAGE_NAME;
  process.env.TURA_TUI_CLIPBOARD_IMAGE_BASE64 = Buffer.from([137, 80, 78, 71]).toString("base64");
  process.env.TURA_TUI_CLIPBOARD_IMAGE_NAME = "clip image.png";
  try {
    const relative = await saveClipboardImageInput(workspace);

    assert.ok(relative);
    assert.match(relative, /^\.tura\/media\/input\//u);
    assert.ok(relative.endsWith("clip-image.png"));
    assert.deepEqual(await readFile(join(workspace, relative)), Buffer.from([137, 80, 78, 71]));
  } finally {
    if (previousImage === undefined) delete process.env.TURA_TUI_CLIPBOARD_IMAGE_BASE64;
    else process.env.TURA_TUI_CLIPBOARD_IMAGE_BASE64 = previousImage;
    if (previousName === undefined) delete process.env.TURA_TUI_CLIPBOARD_IMAGE_NAME;
    else process.env.TURA_TUI_CLIPBOARD_IMAGE_NAME = previousName;
    await rm(workspace, { recursive: true, force: true });
  }
});
