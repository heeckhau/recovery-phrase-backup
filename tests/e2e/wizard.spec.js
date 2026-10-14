// The correctness-critical UI (ROADMAP, "Considerations — UI"): both
// wizards end to end against the release file, under its strict CSP, with
// the checks that matter — the spot check, the recovery test, what gets
// printed, and that nothing secret is left behind.
const { test, expect } = require("@playwright/test");
const { pathToFileURL } = require("node:url");

const URL = pathToFileURL(process.env.PKC_HTML).href;

// Not a published test vector: entropy 3c910ed752a86f14b329e07d459ac618.
const PHRASE = "develop mask remind pipe manage medal smile despair later coast ranch cousin";
const FINGERPRINT = "e3ba7ad6";
const TEST_PHRASE = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

test.beforeEach(async ({ page }) => {
  // Record CSP violations, everything sent to print, and any attempt to use
  // the clipboard or storage, before the page's own script runs.
  await page.addInitScript(() => {
    window.__violations = [];
    window.__prints = [];
    window.__clipboardWrites = 0;
    document.addEventListener("securitypolicyviolation", (e) => window.__violations.push(e.violatedDirective));
    window.print = () => window.__prints.push(document.getElementById("print-area").innerText);
    if (navigator.clipboard) navigator.clipboard.writeText = async () => void window.__clipboardWrites++;
    document.addEventListener("copy", () => void window.__clipboardWrites++);
  });
  page.on("pageerror", (err) => {
    throw err;
  });
  await page.goto(URL);
  await expect(page.locator("#start-backup")).toBeEnabled();
});

test.afterEach(async ({ page }) => {
  const leftovers = await page.evaluate(() => ({
    violations: window.__violations,
    clipboardWrites: window.__clipboardWrites,
    localStorage: localStorage.length,
    sessionStorage: sessionStorage.length,
  }));
  expect(leftovers).toEqual({ violations: [], clipboardWrites: 0, localStorage: 0, sessionStorage: 0 });
});

async function pastePhrase(page, phrase) {
  await page.evaluate((text) => {
    const input = document.querySelector("#b1-grid input");
    const data = new DataTransfer();
    data.setData("text", text);
    input.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true, cancelable: true }));
  }, phrase);
}

async function typeShare(page, entryId, share) {
  const words = share.split(" ");
  const inputs = page.locator(`#${entryId} .word-grid input`);
  for (let i = 0; i < words.length; i++) await inputs.nth(i).fill(words[i]);
  await page.locator(`#${entryId} button.btn-secondary`).click();
}

// Walks the share screens, passing every spot check; returns the shares.
async function writeDownAllShares(page) {
  const shares = [];
  while (await page.locator("#b4").isVisible()) {
    const words = await page.locator("#b4-words .word").allTextContents();
    shares.push(words.join(" "));
    await page.click("#b4-written");
    const positions = (await page.locator("#b4-check-grid .num").allTextContents()).map((n) => Number(n) - 1);
    const inputs = page.locator("#b4-check-grid input");
    for (let i = 0; i < 3; i++) await inputs.nth(i).fill(words[positions[i]]);
    await page.click("#b4-check");
  }
  return shares;
}

async function backUp(page, phrase = PHRASE) {
  await page.click("#start-backup");
  await pastePhrase(page, phrase);
  await page.click("#b1-next");
  await page.click("#b2-next");
  await page.click("#b3-next");
  return writeDownAllShares(page);
}

test("the release file carries no dev-only code", async ({ page }) => {
  await page.goto(`${URL}?test`);
  await expect(page.locator("#home")).toBeVisible();
  await expect(page.locator("#backup")).toBeHidden();
});

test("back up a phrase: check, split, spot-check, test, print", async ({ page }) => {
  await page.click("#start-backup");
  await pastePhrase(page, PHRASE);
  await expect(page.locator("#b1-fingerprint")).toHaveText(FINGERPRINT);
  await expect(page.locator("#b1-warnings .banner")).toHaveCount(0);
  await expect(page.locator("#b1-ack-row")).toBeHidden();
  await page.click("#b1-next");
  await page.click("#b2-next");
  await expect(page.locator("#b3-summary")).toContainText("Any 2 of them");
  await page.click("#b3-next");

  // Share 1: print a blank card, then the filled card through the warning.
  const first = (await page.locator("#b4-words .word").allTextContents()).join(" ");
  await page.click("#b4-print-blank");
  await page.click("#b4-print");
  await expect(page.locator("#print-dialog")).toHaveAttribute("open", "");
  await page.click("#print-confirm");
  const [blank, filled] = await page.evaluate(() => window.__prints);
  for (const word of first.split(" ").slice(3)) expect(blank).not.toContain(`. ${word}`);
  for (const word of first.split(" ")) expect(filled).toContain(word);
  expect(filled).toContain(FINGERPRINT);
  for (const word of PHRASE.split(" ").filter((w) => !first.includes(w))) expect(filled).not.toContain(word);

  // A wrong spot-check word is caught.
  await page.click("#b4-written");
  const positions = (await page.locator("#b4-check-grid .num").allTextContents()).map((n) => Number(n) - 1);
  const words = first.split(" ");
  const inputs = page.locator("#b4-check-grid input");
  await inputs.nth(0).fill(words[positions[0]] === "acid" ? "academic" : "acid");
  await inputs.nth(1).fill(words[positions[1]]);
  await inputs.nth(2).fill(words[positions[2]]);
  await page.click("#b4-check");
  await expect(page.locator("#b4-error")).toContainText(`Word ${positions[0] + 1} doesn't match`);
  await page.click("#b4-show-again");

  const shares = [first, ...(await writeDownAllShares(page)).slice(1)];
  expect(shares).toHaveLength(3);
  expect(shares.every((s) => s.split(" ").length === 20)).toBe(true);

  // The recovery test must reproduce the fingerprint.
  await typeShare(page, "b5-entry", shares[0]);
  await typeShare(page, "b5-entry", shares[2]);
  await expect(page.locator("#b5-result")).toContainText(`Recovery works`);
  await expect(page.locator("#b5-result")).toContainText(FINGERPRINT);
  await page.click("#b5-next");

  await page.click("#b6-print-sheet");
  const sheet = (await page.evaluate(() => window.__prints)).at(-1);
  expect(sheet).toContain(FINGERPRINT);
  for (const word of PHRASE.split(" ")) expect(sheet).not.toMatch(new RegExp(`\\b${word}\\b`));

  await page.click("#b6-finish");
  await expect(page.locator("#home")).toBeVisible();
  const anyText = await page.evaluate(() => [...document.querySelectorAll("input")].some((i) => i.type === "text" && i.value));
  expect(anyText).toBe(false);
});

test("a miscopied share fails the recovery test", async ({ page }) => {
  const shares = await backUp(page);
  const words = shares[0].split(" ");
  [words[5], words[6]] = [words[6], words[5]];
  await typeShare(page, "b5-entry", words.join(" "));
  await expect(page.locator("#b5-entry .error")).toContainText("doesn't check out");
  await expect(page.locator("#b5-next")).toBeHidden();
});

test("recover a phrase from shares, typed with 4-letter prefixes", async ({ page }) => {
  const shares = await backUp(page);
  await page.click("#b5-skip");
  await page.click("#b6-finish");

  await page.click("#start-recover");
  const inputs = page.locator("#r1-entry .word-grid input");
  const words = shares[1].split(" ");
  for (let i = 0; i < words.length; i++) {
    await inputs.nth(i).click();
    await page.keyboard.type(`${words[i].slice(0, 4)} `);
  }
  await page.locator("#r1-entry button.btn-secondary").click();
  await expect(page.locator("#r1-entry p.notice")).toHaveText("You have 1 of the 2 shares needed.");
  await typeShare(page, "r1-entry", shares[2]);
  await expect(page.locator("#r1-entry p.notice")).toContainText("enough");

  await page.click("#r1-recover");
  expect((await page.locator("#r2-words .word").allTextContents()).join(" ")).toBe(PHRASE);
  await expect(page.locator("#r2-fingerprint")).toHaveText(FINGERPRINT);
  await expect(page.locator("#r2-addresses")).toBeHidden();
  await page.click("#r2-yes");
  await expect(page.locator("#r2-addresses")).toBeVisible();
});

test("a wrong SLIP-39 passphrase gives a different fingerprint, not an error", async ({ page }) => {
  const shares = await backUp(page);
  await page.click("#b5-skip");
  await page.click("#b6-finish");
  await page.click("#start-recover");
  await typeShare(page, "r1-entry", shares[0]);
  await typeShare(page, "r1-entry", shares[1]);
  await page.locator("#r1 details summary").click();
  await page.fill("#r1-passphrase", "not the passphrase");
  await page.click("#r1-recover");
  await expect(page.locator("#r2-fingerprint")).not.toHaveText(FINGERPRINT);
  await page.click("#r2-no");
  await expect(page.locator("#r2-mismatch")).toBeVisible();
});

test("test phrases need an explicit acknowledgement; a bad checksum blocks", async ({ page }) => {
  await page.click("#start-backup");
  await pastePhrase(page, TEST_PHRASE);
  await expect(page.locator("#b1-warnings .banner")).toHaveCount(2);
  await expect(page.locator("#b1-next")).toBeDisabled();
  await page.check("#b1-ack");
  await expect(page.locator("#b1-next")).toBeEnabled();

  await pastePhrase(page, TEST_PHRASE.replace(/about$/, "abandon"));
  await expect(page.locator("#b1-error")).toContainText("checksum doesn't match");
  await expect(page.locator("#b1-next")).toBeDisabled();
});

test("schemes that are weaker than they look are flagged", async ({ page }) => {
  await page.click("#start-backup");
  await pastePhrase(page, PHRASE);
  await page.click("#b1-next");
  await page.check("input[name=scheme][value=custom]");
  await page.selectOption("#group-rows .group-threshold-input", "3");
  await expect(page.locator("#b2-guard")).toContainText("every share is needed");
});
