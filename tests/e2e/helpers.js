// Shared by the browser tests: open a page with recorders installed, check
// nothing was left behind, and drive the wizard's word grids.
const { expect } = require("@playwright/test");

// Records CSP violations, everything sent to print, and any attempt to use
// the clipboard or storage, before the page's own script runs.
async function openRecorded(page, url) {
  await page.addInitScript(() => {
    window.__violations = [];
    window.__prints = [];
    window.__clipboardWrites = 0;
    document.addEventListener("securitypolicyviolation", (e) => window.__violations.push(e.violatedDirective));
    window.print = () => {
      const area = document.getElementById("print-area");
      const cells = [...area.querySelectorAll(".words div")].map((cell) => cell.textContent);
      const layout = area.querySelector(".one-per-page") ? "pages" : "sheet";
      window.__prints.push({ text: area.innerText, cells, layout });
    };
    if (navigator.clipboard) navigator.clipboard.writeText = async () => void window.__clipboardWrites++;
    document.addEventListener("copy", () => void window.__clipboardWrites++);
  });
  page.on("pageerror", (err) => {
    throw err;
  });
  await page.goto(url);
  await expect(page.locator("#start-backup")).toBeEnabled();
}

async function expectNothingLeftBehind(page) {
  const leftovers = await page.evaluate(() => ({
    violations: window.__violations,
    clipboardWrites: window.__clipboardWrites,
    localStorage: localStorage.length,
    sessionStorage: sessionStorage.length,
  }));
  expect(leftovers).toEqual({ violations: [], clipboardWrites: 0, localStorage: 0, sessionStorage: 0 });
}

async function pastePhrase(page, phrase) {
  await page.evaluate((text) => {
    const input = document.querySelector("#b1-grid input");
    const data = new DataTransfer();
    data.setData("text", text);
    // Firefox empties the clipboardData of a paste event made by a script,
    // so it is set on the event itself rather than through the constructor.
    const event = new ClipboardEvent("paste", { bubbles: true, cancelable: true });
    Object.defineProperty(event, "clipboardData", { value: data });
    input.dispatchEvent(event);
  }, phrase);
}

async function typeShare(page, entryId, share) {
  const words = share.split(" ");
  const inputs = page.locator(`#${entryId} .word-grid input`);
  for (let i = 0; i < words.length; i++) await inputs.nth(i).fill(words[i]);
  await page.locator(`#${entryId} button.btn-secondary`).click();
}

// The spot check moves on by itself once all three words match.
async function waitForSpotCheckToPass(page) {
  await page.waitForFunction(() => document.getElementById("b4").hidden || document.getElementById("b4-verify").hidden);
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
    await waitForSpotCheckToPass(page);
  }
  return shares;
}

module.exports = { openRecorded, expectNothingLeftBehind, pastePhrase, typeShare, waitForSpotCheckToPass, writeDownAllShares };
