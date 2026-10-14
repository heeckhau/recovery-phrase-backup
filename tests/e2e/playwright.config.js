// Runs against the built release file, opened from disk exactly as a user
// would: build it first (scripts/build.py or scripts/build-reproducible.sh),
// or point PKC_HTML at another copy.
const path = require("node:path");
const { defineConfig, devices } = require("@playwright/test");

process.env.PKC_HTML ||= path.resolve(__dirname, "../../dist/private-key-convertor.html");

module.exports = defineConfig({
  testDir: ".",
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  reporter: process.env.CI ? "list" : "line",
  use: { trace: "retain-on-failure" },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
});
