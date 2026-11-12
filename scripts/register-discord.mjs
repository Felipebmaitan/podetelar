// Run explicitly after configuring a Discord app. Never print the bot token or response bodies.
const application = process.env.DISCORD_APPLICATION_ID;
const guilds = (process.env.DISCORD_GUILD_IDS ?? "")
  .split(",")
  .map((id) => id.trim());
const token = process.env.DISCORD_BOT_TOKEN;
if (
  !/^\d{17,20}$/.test(application ?? "") ||
  !guilds.length ||
  !guilds.every((id) => /^\d{17,20}$/.test(id)) ||
  !token
)
  throw new Error(
    "Set DISCORD_APPLICATION_ID, DISCORD_GUILD_IDS and DISCORD_BOT_TOKEN locally.",
  );
for (const guild of guilds) {
  for (const name of ["stream", "telar"]) {
    let response;
    try {
      response = await fetch(
        `https://discord.com/api/v10/applications/${application}/guilds/${guild}/commands`,
        {
          method: "POST",
          headers: {
            Authorization: `Bot ${token}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            name,
            type: 1,
            description: "Create a private FreedomStream room for friends",
            options: [],
          }),
          signal: AbortSignal.timeout(10000),
        },
      );
    } catch {
      throw new Error("Discord command registration connection failed.");
    }
    if (!response.ok)
      throw new Error(
        `Discord command registration failed (HTTP ${response.status}).`,
      );
    await response.body?.cancel();
  }
}
console.log("Registered /stream and /telar in the configured Discord servers.");
