---
name: nddev-browseruse
description: Drive the owner's own Chrome with Playwright code through the nddev-browseruse CLI, on this computer, another of the owner's devices or Chrome on the owner's Android phone. The agent's tabs stay in its own tab group. Use it instead of fetch or curl for JS-heavy pages, signed-in sites, cookie walls, forms, downloads, screenshots and recordings, and to check our own sites and apps the way a user sees them. Load this skill before the first nddev-browseruse command.
---

## Read the full guide once per session

Before the first command in a session, run:

```bash
nddev-browseruse skill
```

Read the whole output. Do not pipe it through `head`, `tail` or any other truncation: the rules
are spread through the guide, not only at the top. It covers sessions, the observe, act and
observe loop, helpers, time limits, devices, phones, bot checks and the rules. Read it once per
session; later commands in the same session do not need it again.

## Minimal example, after reading the guide

```bash
nddev-browseruse status
nddev-browseruse session new docs
nddev-browseruse -s docs -e 'state.page = await context.newPage(); await state.page.goto("https://example.com"); return await snapshot({ page: state.page })'
nddev-browseruse session close docs
```

- Use one session per task, named with a short word for the task or site. The name is the
  agent's tab group in the owner's browser.
- Wrap `-e` code in single quotes so the shell leaves `$`, backticks and backslashes alone. Use
  double quotes for strings inside the JavaScript. Put longer code in a file and pass `-f`.
- Add `--device <name>` to run on another of the owner's computers, or `--phone <serial>` for
  the owner's phone, when the owner serves it.
- `--local` bypasses configured device/phone defaults. Peer forwarding needs CLI 0.2.5 or later.
  Nonempty legacy NDS routing/runtime or phone ADB endpoint environment settings require explicit
  migration to installed configuration; never silently retry a refused action elsewhere.
- Close the session when the task is done. That closes its tabs too.
- Workflow recording needs the owner's Full observation choice. Off, Standard and changes to
  private sites stop it; Full does not restart it. Images are not pixel-redacted. Export needed
  frames before closing the session; `outputDir` stays within the session's artifacts.

## Rules

- A CAPTCHA or sign-in check belongs to the owner. Never solve or bypass one: call
  `notifyOwner` and continue with other work.
- If `nddev-browseruse status` does not answer `enabled: true`, ask the owner to open the browser
  and check the extension. Never change the owner's browser or phone settings yourself.
- If `nddev-browseruse` is not on the `PATH`, tell the owner. Use the product's installation instructions; do not install or replace the owner's host without authorization.
