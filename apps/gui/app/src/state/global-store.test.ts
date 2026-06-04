import type { Session } from "@tura/gateway-sdk";
import { describe, expect, test } from "bun:test";
import {
  SESSION_FALLBACK_NAME_MAX_LENGTH,
  sessionFallbackNameFromInput,
  sessionTitle,
  withSessionFallbackName,
} from "./global-store";

describe("session fallback names", () => {
  test("uses the fixed-length head of the user input", () => {
    const input = `  创建一个用于验证 gateway 空会话名称 fallback 的新会话

并保留后续 runtime task_status summary 覆盖的空间`;

    const fallback = sessionFallbackNameFromInput(input);

    expect(fallback).toBe(
      Array.from(
        "创建一个用于验证 gateway 空会话名称 fallback 的新会话 并保留后续 runtime task_status summary 覆盖的空间",
      )
        .slice(0, SESSION_FALLBACK_NAME_MAX_LENGTH)
        .join(""),
    );
  });

  test("fills empty gateway session names without overwriting real names", () => {
    const blank: Session = {
      id: "s-empty-name",
      name: "",
      session_display_name: "",
      plan_summary: "",
      status: "idle",
    };
    const filled = withSessionFallbackName(
      blank,
      "头部输入会成为前端临时会话名",
    );

    expect(filled.name).toBe("头部输入会成为前端临时会话名");
    expect(sessionTitle(filled)).toBe("头部输入会成为前端临时会话名");

    const named: Session = {
      id: "s-named",
      name: "gateway name",
      status: "idle",
    };

    expect(withSessionFallbackName(named, "用户输入").name).toBe(
      "gateway name",
    );
  });
});
