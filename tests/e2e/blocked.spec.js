// Browsers that block what the page needs, as Tor Browser does at its Safer
// and Safest security levels (on Tails, say): the page says what to change
// instead of showing buttons that never enable.
const { test, expect } = require("@playwright/test");
const { pathToFileURL } = require("node:url");

const URL = pathToFileURL(process.env.RPB_HTML).href;

test("without WebAssembly, the page says to lower Tor Browser's security level", async ({ page }) => {
  await page.addInitScript(() => {
    delete window.WebAssembly;
  });
  await page.goto(URL);
  await expect(page.locator("#load-error")).toContainText("set the security level to Standard");
  await expect(page.locator("#start-backup")).toBeDisabled();
});

// Firefox only: Chromium under Playwright stops scripts but doesn't render
// <noscript>. Playwright's text locators skip <noscript>, so this checks the
// banner's text and that it takes up room on the page.
test.describe("without JavaScript", () => {
  test.use({ javaScriptEnabled: false });

  test("the page says to lower Tor Browser's security level", async ({ page, browserName }) => {
    test.skip(browserName !== "firefox", "Chromium under Playwright doesn't render <noscript>");
    await page.goto(URL);
    const banner = page.locator("noscript .banner");
    await expect(banner).toContainText("This page needs JavaScript and WebAssembly");
    expect((await banner.boundingBox())?.height).toBeGreaterThan(0);
  });
});
