import assert from "node:assert/strict";
import { test } from "node:test";
import { registerDiscordCommands } from "./discord-commands.mjs";

test("global registration restricts commands to guild installations and preserves unrelated commands", async () => {
  const calls = [];
  await registerDiscordCommands(
    "111111111111111111",
    "test-token",
    async (url, input) => {
      calls.push({ url, ...input });
      return new Response("{}", { status: 200 });
    },
  );
  assert.equal(calls.length, 2);
  assert.deepEqual(
    calls.map((call) => JSON.parse(call.body).name),
    ["stream", "telar"],
  );
  for (const call of calls) {
    assert.equal(
      call.url,
      "https://discord.com/api/v10/applications/111111111111111111/commands",
    );
    assert.equal(call.method, "POST");
    const body = JSON.parse(call.body);
    assert.deepEqual(body.integration_types, [0]);
    assert.deepEqual(body.contexts, [0]);
    assert.ok(!call.url.includes("test-token"));
  }
});
test("registration validates identity and redacts connection and API error bodies", async () => {
  await assert.rejects(
    registerDiscordCommands("not-an-id", "test-token"),
    /DISCORD/,
  );
  await assert.rejects(
    registerDiscordCommands("111111111111111111", "test-token", async () => {
      throw Error("secret-value");
    }),
    { message: "Discord command registration connection failed." },
  );
  await assert.rejects(
    registerDiscordCommands(
      "111111111111111111",
      "test-token",
      async () => new Response("secret-value", { status: 403 }),
    ),
    { message: "Discord command registration failed (HTTP 403)." },
  );
});
