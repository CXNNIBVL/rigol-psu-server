import { defineConfig, devices } from '@playwright/test';

const PORT = 18099;

/**
 * End-to-end tests drive the real binary: real Rust server, real WebSocket,
 * real SPA, nothing mocked. `--simulate 3` stands in for the bench, which is
 * what makes this runnable in CI at all — and, more importantly, what makes the
 * failure states reachable. A supply going offline is a one-line fault
 * injection here and a trip to the lab otherwise.
 */
export default defineConfig({
  testDir: './e2e',
  fullyParallel: false,
  workers: 1,
  forbidOnly: !!process.env.CI,
  retries: 0,
  reporter: process.env.CI ? 'list' : 'line',
  use: {
    baseURL: `http://127.0.0.1:${PORT}`,
    trace: 'on-first-retry',
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  webServer: {
    command: `cargo run -p server --features simulation -- --simulate 3 --bind 127.0.0.1:${PORT}`,
    cwd: '..',
    url: `http://127.0.0.1:${PORT}/api/state`,
    reuseExistingServer: !process.env.CI,
    timeout: 180_000,
    stdout: 'pipe',
    stderr: 'pipe',
  },
});
