import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./native-e2e",
  workers: 1,
  retries: 0,
  timeout: 150_000,
  reporter: "list",
  use: {
    baseURL: "http://localhost:3000",
    channel: "chrome",
    headless: true,
    trace: "off",
    screenshot: "off",
  },
});
