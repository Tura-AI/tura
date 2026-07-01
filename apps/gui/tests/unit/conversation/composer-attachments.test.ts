import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const composerSource = readFileSync(
  resolve(import.meta.dir, "../../../app/src/conversation/composer.tsx"),
  "utf8",
);
const composerCss = readFileSync(
  resolve(import.meta.dir, "../../../app/src/styles/parts/surfaces/composer-content.css"),
  "utf8",
);

describe("composer attachment drag and sizing", () => {
  test("owns file drag and drop on the full composer surface", () => {
    expect(composerSource).toContain("function composerDataTransferHasFiles");
    expect(composerSource).toContain("function handleComposerDragEnter");
    expect(composerSource).toContain("function handleComposerDrop");
    expect(composerSource).toContain('classNames("bottom-composer composer"');
    expect(composerSource).toContain("onDragEnter={handleComposerDragEnter}");
    expect(composerSource).toContain("onDrop={handleComposerDrop}");

    const inputBlock = composerSource.slice(
      composerSource.indexOf('class="composer-input"'),
      composerSource.indexOf('class="composer-toolbar"'),
    );
    expect(inputBlock).not.toContain("onDrop=");
  });

  test("keeps Enter submission semantics stable while sessions are running", () => {
    expect(composerSource).toContain(
      'if (event.key !== "Enter" || event.shiftKey || event.isComposing) {',
    );
    expect(composerSource).toContain("event.preventDefault();");
    const submitFromControlBlock = composerSource.slice(
      composerSource.indexOf("function submitFromControl"),
      composerSource.indexOf("function submitFromKeyboard"),
    );
    const submitFromKeyboardBlock = composerSource.slice(
      composerSource.indexOf("function submitFromKeyboard"),
      composerSource.indexOf("const sendButtonTitle"),
    );
    expect(submitFromControlBlock).not.toContain("props.running");
    expect(submitFromControlBlock).not.toContain("props.onStop");
    expect(submitFromKeyboardBlock).not.toContain("props.running");
    expect(submitFromKeyboardBlock).not.toContain("props.onStop");
  });

  test("keeps attachment chips at the current composer text height", () => {
    expect(composerCss).toContain(".composer-attachment-token");
    expect(composerCss).toContain("height: 1lh;");
    expect(composerCss).toContain("line-height: 1;");
    expect(composerCss).toContain(".composer-attachment-token button");
    expect(composerCss).toContain("height: 1lh;");
    expect(composerCss).toContain(".composer-attachment-token img");
    expect(composerCss).toContain("width: 1em;");
    expect(composerCss).toContain("height: 1em;");
  });
});
