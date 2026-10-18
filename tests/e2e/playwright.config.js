// Runs against the built release file, opened from disk exactly as a user
// would: build it first (scripts/build.py or scripts/build-reproducible.sh),
// or point RPB_HTML at another copy.
const path = require("node:path");
const { defineConfig, devices } = require("@playwright/test");

process.env.RPB_HTML ||= path.resolve(__dirname, "../../dist/recovery-phrase-backup.html");
process.env.RPB_DEMO_HTML ||= path.resolve(__dirname, "../../dist/demo.html");

module.exports = defineConfig({
  testDir: ".",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? "list" : "line",
  use: { trace: "retain-on-failure" },
  // Firefox stands in for Tor Browser, the browser on Tails, which the
  // README recommends.
  projects: [
    { name: "chromium", use: { ...devices["Desktop Chrome"] } },
    { name: "firefox", use: { ...devices["Desktop Firefox"] } },
  ],
});
