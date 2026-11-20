// Bot credentials are used only by this explicit registration step, never by the server.
import { registerDiscordCommands } from "./discord-commands.mjs";
await registerDiscordCommands(
  process.env.DISCORD_APPLICATION_ID,
  process.env.DISCORD_BOT_TOKEN,
);
console.log(
  "Registered global /stream and /telar commands for all server installations.",
);
