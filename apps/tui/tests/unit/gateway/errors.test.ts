import test from "node:test";
import assert from "node:assert/strict";
import { setLanguage } from "../../../src/i18n.js";
import { GatewayHttpError, userFacingError } from "../../../src/gateway/errors.js";

test("formats gateway disconnects without exposing raw fetch errors", () => {
  setLanguage("en");
  const message = userFacingError(
    new GatewayHttpError(0, "http://127.0.0.1:4126/provider", "fetch failed"),
  );
  assert.match(message, /Gateway connection was lost/);
  assert.doesNotMatch(message, /GatewayHttpError|file:|\.js:\d+/);
});

test("formats provider HTTP errors with provider and code details", () => {
  setLanguage("en");
  const message = userFacingError(
    new GatewayHttpError(
      429,
      "http://127.0.0.1:4126/session/s1/prompt_async",
      "gateway returned HTTP 429",
      JSON.stringify({
        provider: "openai",
        code: "rate_limit_exceeded",
        message: "quota exhausted",
      }),
    ),
  );
  assert.match(message, /HTTP 429/);
  assert.match(message, /provider: openai/);
  assert.match(message, /code: rate_limit_exceeded/);
  assert.match(message, /quota exhausted/);
});

test("formats non-json gateway bodies compactly", () => {
  setLanguage("en");
  const message = userFacingError(
    new GatewayHttpError(
      500,
      "http://127.0.0.1:4126/provider",
      "gateway returned HTTP 500",
      "boom",
    ),
  );
  assert.match(message, /HTTP 500/);
  assert.match(message, /boom/);
});
