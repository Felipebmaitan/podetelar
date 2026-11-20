import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./native-soak",
  workers: 1,
  retries: 0,
  timeout: 1_950_000,
  reporter: "list",
  use: { baseURL: process.env.E2E_BASE_URL ?? "http://localhost:3000" },
});
