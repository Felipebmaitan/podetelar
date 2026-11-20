# Working on PodeTelar

- Read `CONTEXT.md` and the relevant guides in `docs/` before changing a feature's scope.
- Use a separate branch and PR for each change. Autonomous implementation and merging are authorized after acceptance checks and approval from a fresh independent review subagent. Address review findings before merging. Do not replace unavailable hardware or deployment verification with claims based only on compilation or mocks.
- Keep application code MIT compatible. Bundled dependencies retain their own licenses; qualify native runtime distribution before introducing it.
- Use Node 24 LTS and npm workspaces. `npm run check` covers formatting, types, API tests and builds. The documented Docker smoke and browser checks exercise actual HTTP and UI behavior.
- Never open a visible terminal or console window. Run necessary build/test/development commands through hidden, noninteractive processes; shipped executables must launch as GUI applications.
- Ordinary development permission prompts are preauthorized, including network/firewall permission for PodeTelar and the local test SFU. Handle supported prompts directly. If a protected dialog cannot be targeted or administrator rights are unavailable, report the tool limitation and continue independent work.
- Keep invitation/session secrets and media server credentials out of logs, URLs sent to the server, fixtures committed with real secrets, and browser local storage. Generated `.env` and room databases are local files.
- Distinguish proposed media capabilities from verified behavior. Claims about codecs, hardware support, audio exclusion and capture modes require actual acceptance checks on the relevant platform and hardware.
- Preview capture is once on stream start, then at most once per minute. Watching a stream must be an actual subscription; hiding its video element does not stop its traffic.
