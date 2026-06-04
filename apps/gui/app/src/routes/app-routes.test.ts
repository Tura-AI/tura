import { describe, expect, test } from "bun:test";
import { PRODUCT_ROUTE_PATHS } from "./route-paths";

describe("product route table", () => {
  test("covers core Multica-compatible routes", () => {
    expect(PRODUCT_ROUTE_PATHS).toContain("/:workspace/issues/:id");
    expect(PRODUCT_ROUTE_PATHS).toContain("/:workspace/agents/:id");
    expect(PRODUCT_ROUTE_PATHS).toContain("/:workspace/autopilots/:id");
    expect(PRODUCT_ROUTE_PATHS).toContain("/:workspace/settings");
  });

  test("keeps public routes explicit", () => {
    expect(PRODUCT_ROUTE_PATHS).toContain("/download");
    expect(PRODUCT_ROUTE_PATHS).toContain("/contact-sales");
  });
});
