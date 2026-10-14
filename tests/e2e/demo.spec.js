// The online demo (dist/demo.html): the same page with DEMO_MODE on. It must
// accept only its published sample phrases, refuse everything else, and
// mark every printout as a demo.
const { test, expect } = require("@playwright/test");
const { pathToFileURL } = require("node:url");
const { openRecorded, expectNothingLeftBehind, pastePhrase, typeShare, writeDownAllShares } = require("./helpers");

const URL = pathToFileURL(process.env.PKC_DEMO_HTML).href;
const SAMPLE_12 = "leopard involve vital powder diary motion comfort foil width later gasp year";
const SAMPLE_FINGERPRINT = "4d7461d8";

test.beforeEach(async ({ page }) => {
  await openRecorded(page, URL);
});

test.afterEach(async ({ page }) => {
  await expectNothingLeftBehind(page);
});

test("says it is a demo", async ({ page }) => {
  await expect(page).toHaveTitle(/\(demo\)$/);
  await expect(page.locator("#demo-banner")).toBeVisible();
  await expect(page.locator("#online-banner")).toBeHidden();
});

test("refuses a phrase that isn't one of its samples", async ({ page }) => {
  await page.click("#start-backup");
  await pastePhrase(page, "develop mask remind pipe manage medal smile despair later coast ranch cousin");
  await expect(page.locator("#b1-error")).toContainText("only works with the sample phrases");
  await expect(page.locator("#b1-check")).toBeHidden();
  await expect(page.locator("#b1-next")).toBeDisabled();
});

test("backs up a sample phrase, and marks every printout as a demo", async ({ page }) => {
  await page.click("#start-backup");
  await page.click('[data-sample="24"]');
  await expect(page.locator("#b1-grid input")).toHaveCount(24);
  await page.click('[data-sample="12"]');
  await expect(page.locator("#b1-fingerprint")).toHaveText(SAMPLE_FINGERPRINT);
  // The sample is a known phrase, but in the demo one note says so instead
  // of the warning and its acknowledgement.
  await expect(page.locator("#b1-demo-note")).toBeVisible();
  await expect(page.locator("#b1-warnings .banner")).toHaveCount(0);
  await expect(page.locator("#b1-ack-row")).toBeHidden();
  await page.click("#b1-next");
  await page.click("#b2-next");
  await page.check('input[name="method"][value="print"]');
  await page.click("#b3-next");
  await page.click("#b4-print");
  await page.click("#print-confirm");
  const shares = await writeDownAllShares(page);
  await typeShare(page, "b5-entry", shares[0]);
  await typeShare(page, "b5-entry", shares[1]);
  await expect(page.locator("#b5-result")).toContainText("Recovery works");
  await page.click("#b5-next");
  await page.click("#b6-print-sheet");
  const [cards, sheet] = await page.evaluate(() => window.__prints);
  expect(cards.text.match(/DEMO, NOT A REAL BACKUP/g)).toHaveLength(3);
  expect(sheet.text).toContain("DEMO, NOT A REAL BACKUP");
});

test("recovers its sample shares", async ({ page }) => {
  await page.click("#start-recover");
  await page.click('[data-share="0"]');
  await page.click('[data-share="2"]');
  await expect(page.locator("#r1-entry p.notice")).toContainText("enough");
  await page.click("#r1-recover");
  expect((await page.locator("#r2-words .word").allTextContents()).join(" ")).toBe(SAMPLE_12);
  await expect(page.locator("#r2-fingerprint")).toHaveText(SAMPLE_FINGERPRINT);
});

test("refuses to recover anything but its samples", async ({ page }) => {
  // The official 128-bit SLIP-39 vector: valid shares of a phrase that isn't a demo sample.
  await page.click("#start-recover");
  await typeShare(page, "r1-entry", "duckling enlarge academic academic agency result length solution fridge kidney coal piece deal husband erode duke ajar critical decision keyboard");
  await page.locator("#r1 details summary").click();
  await page.fill("#r1-passphrase", "TREZOR");
  await page.click("#r1-recover");
  await expect(page.locator("#r1-error")).toContainText("only recovers backups of its sample phrases");
  await expect(page.locator("#r2")).toBeHidden();
});
