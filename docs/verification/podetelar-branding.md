# PodeTelar branding verification

The display name is **PodeTelar**, with uppercase P and T. The supplied
`assets/PodeTelar.png` is shared by the README, web favicon and brand marks;
`scripts/make-windows-icon.py` derives the Windows ICO and native RGBA assets.
Existing web and native palettes are preserved. Internal protocol, executable,
package, cache and settings identifiers remain compatible with earlier versions.

## Checks

- Node 24.11.1 `npm run check`: formatting, types, 61 server tests, two web tests,
  five configuration tests and production builds passed.
- `docker compose up -d --build` and `npm run smoke -- --restart-app`: actual
  built HTTP assets, admission, authorization, persisted sessions and SFU access
  passed. The image includes the source artwork before compiling the web app.
- Native workspace Clippy, build and tests passed: ten portable tests and 73
  publisher tests, with four pre-existing ignored publisher tests. The extra
  runtime registry test and three installed-launcher tests passed as well.
- Independent GPT-6.1-Sol high-reasoning review approved the permanent change
  after Docker asset inclusion and renamed smoke/resource expectations were fixed.

## Screenshots

Browser captures use actual local HTTP and LiveKit services, without mocked routes.
The standard local SFU has no private traffic collector, so Usage accurately shows
collection unavailable. These captures verify branding and layout, not traffic
accounting or production deployment.

| Web screen              | Screenshot                                                      |
| ----------------------- | --------------------------------------------------------------- |
| Administrator login     | [Login](images/podetelar/admin-login.png)                       |
| Rooms, expanded         | [Rooms](images/podetelar/admin-rooms.png)                       |
| New room dialog         | [New room](images/podetelar/admin-new-room.png)                 |
| Usage dashboard         | [Usage](images/podetelar/admin-usage.png)                       |
| Usage custom period     | [Custom period](images/podetelar/admin-usage-custom-period.png) |
| Room access management  | [Access](images/podetelar/manage-room-access.png)               |
| Invitation              | [Invitation](images/podetelar/invitation.png)                   |
| Direct download landing | [Download](images/podetelar/download.png)                       |

Native captures use the actual egui framebuffer at 1280×720, including a real
join to the isolated API/SFU. The Windows capture helper was unavailable (native
pipe error 2 after retries and reset). A temporary local screenshot harness was
removed before restoring and rebuilding the production source. These images
exclude Windows title-bar/taskbar pixels and do not qualify media or hardware.

| Windows app screen | Screenshot                                           |
| ------------------ | ---------------------------------------------------- |
| Onboarding         | [Onboarding](images/podetelar/native-onboarding.png) |
| Joined room        | [Room](images/podetelar/native-room.png)             |
| Advanced settings  | [Advanced](images/podetelar/native-advanced.png)     |

Unsigned distribution, fresh-machine installation and existing hardware/network
qualification gates remain separate. No release assets or public update feed are
published by this branding change.
