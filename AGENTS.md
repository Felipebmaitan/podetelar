# Working on FreedomStream

- Read `CONTEXT.md` and `docs/implementation-plan.md` before changing a feature's scope.
- Phase 1 is the initial MIT-licensed commit on `main`. Each later phase must be its own branch and PR. Felipe authorized autonomous progression through all phases: before merging each PR, run its acceptance checks and request a new review subagent using GPT-6.1-Sol with high reasoning. Address findings and obtain approval, merge, then start the next phase. Do not replace unavailable hardware/deployment verification with claims based only on compilation or mocks.
- Keep application code MIT compatible. Bundled dependencies retain their own licenses; qualify native runtime distribution before introducing it.
- Use Node 24 LTS and npm workspaces. `npm run check` covers formatting, types, API tests and builds. The documented Docker smoke and browser checks exercise actual HTTP and UI behavior.
- Never open a visible terminal or console window. Run necessary build/test/development commands through hidden, noninteractive processes; shipped executables must launch as GUI applications.
- Felipe preauthorized ordinary permission prompts needed for this development, including network/firewall permission for FreedomStream and the local test SFU. Handle supported prompts without asking him to click them. If a protected dialog cannot be targeted or administrator rights are unavailable, report the tool limitation and continue independent work.
- Keep invitation/session secrets and media server credentials out of logs, URLs sent to the server, fixtures committed with real secrets, and browser local storage. Generated `.env` and room databases are local files.
- Distinguish proposed media capabilities from verified behavior. Do not report native HEVC, AMD/NVIDIA qualification, audio exclusion or 4K120 as implemented until their phase's actual acceptance checks pass.
- Preview capture is once on stream start, then at most once per minute. Watching a stream must be an actual subscription; hiding its video element does not stop its traffic.
