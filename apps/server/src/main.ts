import { buildApp } from "./app.js";
import { loadConfig } from "./config.js";

const config = loadConfig();
const app = await buildApp(config, { logger: true });
for (const signal of ["SIGINT", "SIGTERM"] as const) {
  process.once(signal, () => {
    void app.close().catch(() => {
      process.exitCode = 1;
    });
  });
}
await app.listen({ host: config.host, port: config.port });
