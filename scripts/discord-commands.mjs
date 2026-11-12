/** Register only PodeTelar's two global, guild-installed commands; preserve other commands. */
export async function registerDiscordCommands(
  application,
  token,
  request = fetch,
) {
  if (
    !/^\d{17,20}$/.test(application ?? "") ||
    typeof token !== "string" ||
    !token
  )
    throw new Error(
      "Set DISCORD_APPLICATION_ID and DISCORD_BOT_TOKEN locally.",
    );
  for (const name of ["stream", "telar"]) {
    let response;
    try {
      response = await request(
        `https://discord.com/api/v10/applications/${application}/commands`,
        {
          method: "POST",
          headers: {
            Authorization: `Bot ${token}`,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            name,
            type: 1,
            description: "Create a private PodeTelar room for friends",
            options: [],
            integration_types: [0],
            contexts: [0],
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
