import { describe, expect, test } from "bun:test";
import { composerActionState } from "../../../app/src/conversation/composer-action";

describe("composer busy action contract", () => {
  test("shows stop for a busy session with no draft content", () => {
    expect(
      composerActionState({
        text: "",
        imageCount: 0,
        running: true,
        submitting: false,
        submitDisabled: false,
        hasStopHandler: true,
      }),
    ).toEqual({ kind: "stop", disabled: false });
  });

  test("keeps append-send for a busy session with draft text", () => {
    expect(
      composerActionState({
        text: "add this detail",
        imageCount: 0,
        running: true,
        submitting: false,
        submitDisabled: false,
        hasStopHandler: true,
      }),
    ).toEqual({ kind: "send", disabled: false });
  });

  test("keeps append-send for a busy session with attachments", () => {
    expect(
      composerActionState({
        text: "",
        imageCount: 1,
        running: true,
        submitting: false,
        submitDisabled: false,
        hasStopHandler: true,
      }),
    ).toEqual({ kind: "send", disabled: false });
  });

  test("keeps idle empty drafts disabled", () => {
    expect(
      composerActionState({
        text: "",
        imageCount: 0,
        running: false,
        submitting: false,
        submitDisabled: false,
        hasStopHandler: true,
      }),
    ).toEqual({ kind: "send", disabled: true });
  });

  test("does not show stop unless the caller reports a busy session", () => {
    expect(
      composerActionState({
        text: "",
        imageCount: 0,
        running: false,
        submitting: false,
        submitDisabled: false,
        hasStopHandler: true,
      }),
    ).toEqual({ kind: "send", disabled: true });
  });
});
