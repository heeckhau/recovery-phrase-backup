// The landing page (dist/site/): no script, a strict CSP that holds, a
// working link to the demo, and an animation that respects reduced motion.
const { test, expect } = require("@playwright/test");
const path = require("node:path");
const { pathToFileURL } = require("node:url");

const SITE = process.env.RPB_SITE || path.resolve(__dirname, "../../dist/site");

test("the landing page links to a working demo and holds its CSP", async ({ page }) => {
  await page.addInitScript(() => {
    window.__violations = [];
    document.addEventListener("securitypolicyviolation", (e) => window.__violations.push(e.violatedDirective));
  });
  await page.goto(pathToFileURL(path.join(SITE, "index.html")).href);
  await expect(page.locator("h1")).toContainText("Recovery Phrase Backup");
  expect(await page.locator("script").count()).toBe(0);
  expect(await page.evaluate(() => document.getAnimations().length)).toBeGreaterThan(0);
  await expect(page.locator("#download")).toBeAttached();
  expect(await page.evaluate(() => window.__violations)).toEqual([]);

  await page.click("text=Try the demo");
  await expect(page).toHaveTitle(/\(demo\)$/);
  await expect(page.locator("#demo-banner")).toBeVisible();
});

test("with reduced motion, the story is told without the animation", async ({ page }) => {
  await page.emulateMedia({ reducedMotion: "reduce" });
  await page.goto(pathToFileURL(path.join(SITE, "index.html")).href);
  await expect(page.locator("figure svg")).toBeHidden();
  await expect(page.locator(".story li")).toHaveCount(5);
});
