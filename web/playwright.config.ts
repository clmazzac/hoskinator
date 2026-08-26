import { defineConfig, devices } from "@playwright/test";

// The port the end-to-end run serves the UI on, kept off 5173 so a dev server can stay up.
const PORT = Number(process.env.E2E_PORT ?? 5199);

// One daemon and one resume.yaml back every test, so the tests take turns.
export default defineConfig({
  testDir: "./tests",
  fullyParallel: false,
  workers: 1,
  reporter: process.env.CI ? "line" : "list",
  use: {
    baseURL: `http://localhost:${PORT}`,
    trace: "retain-on-failure",
  },
  projects: [{ name: "chromium", use: { ...devices["Desktop Chrome"] } }],
  webServer: {
    command: `npx vite --port ${PORT} --strictPort`,
    url: `http://localhost:${PORT}`,
    reuseExistingServer: true,
    timeout: 60_000,
  },
});
