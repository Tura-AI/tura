import { describe, expect, test } from "bun:test";
import { createQueryCache, queryKeys, serializeQueryKey } from "./query-keys";

describe("query keys", () => {
  test("serialize keys deterministically", () => {
    expect(serializeQueryKey(queryKeys.sessions("C:/work/tura"))).toBe(
      "execution\u001fsessions\u001fC:/work/tura",
    );
  });

  test("cache invalidates by prefix", () => {
    const cache = createQueryCache();
    cache.set(queryKeys.sessions("one"), ["a"]);
    cache.set(queryKeys.messages("message-one"), ["b"]);
    cache.invalidate(["execution", "sessions"]);
    expect(cache.get(queryKeys.sessions("one"))).toBeUndefined();
    expect(cache.get(queryKeys.messages("message-one"))).toEqual(["b"]);
  });
});
