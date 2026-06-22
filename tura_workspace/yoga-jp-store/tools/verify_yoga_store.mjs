import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";
import path from "node:path";
import fs from "node:fs/promises";

const require = createRequire(import.meta.url);
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const workspace = path.resolve(root, "..", "..");
const playwrightRequire = createRequire(path.join(workspace, "scripts", "packages", "playwright_node", "package.json"));
const { chromium } = playwrightRequire("playwright");
const outDir = path.join(root, "test-results");
await fs.mkdir(outDir, { recursive: true });

const pagePath = path.join(root, "index.html");
const url = `file:///${pagePath.replace(/\\/g, "/")}`;

const browser = await chromium.launch({ headless: true });
const desktop = await browser.newPage({ viewport: { width: 1440, height: 1100 }, deviceScaleFactor: 1 });
await desktop.goto(url);
await desktop.waitForLoadState("networkidle");
await desktop.screenshot({ path: path.join(outDir, "desktop.png"), fullPage: true });

const productCount = await desktop.locator(".product-tile").count();
if (productCount !== 4) throw new Error(`Expected 4 products, got ${productCount}`);

await desktop.locator('[data-filter="mat"]').click();
const filteredCount = await desktop.locator(".product-tile").count();
if (filteredCount !== 1) throw new Error(`Expected 1 mat product, got ${filteredCount}`);

await desktop.locator('[data-filter="all"]').click();
await desktop.locator('[data-add-product="mat-sui"]').first().click();
await desktop.locator("[data-close-cart]").first().click();
await desktop.locator('[data-add-product="wear-kiri"]').first().click();
const cartCount = await desktop.locator("[data-cart-count]").innerText();
if (cartCount.trim() !== "2") throw new Error(`Expected cart count 2, got ${cartCount}`);
await desktop.screenshot({ path: path.join(outDir, "cart-open.png"), fullPage: true });

await desktop.locator("[data-close-cart]").first().click();
await desktop.locator('[data-view-product="prop-koishi"]').click();
const detailTitle = await desktop.locator("#drawer-title").innerText();
if (!detailTitle.includes("Koishi")) throw new Error(`Expected Koishi detail drawer, got ${detailTitle}`);
await desktop.screenshot({ path: path.join(outDir, "product-detail.png"), fullPage: true });

const mobile = await browser.newPage({ viewport: { width: 390, height: 920 }, isMobile: true });
await mobile.goto(url);
await mobile.waitForLoadState("networkidle");
await mobile.screenshot({ path: path.join(outDir, "mobile.png"), fullPage: true });
const mobileOverflow = await mobile.evaluate(() => document.documentElement.scrollWidth > window.innerWidth + 1);
if (mobileOverflow) throw new Error("Mobile layout has horizontal overflow");

await browser.close();

console.log(JSON.stringify({ ok: true, url, screenshots: ["desktop.png", "cart-open.png", "mobile.png"] }, null, 2));
