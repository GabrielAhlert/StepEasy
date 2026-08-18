# Security Policy

## Reporting a vulnerability

Please do **not** open a public issue.

Use GitHub's [private vulnerability
reporting](https://github.com/GabrielAhlert/StepEasy/security/advisories/new),
or e-mail <gabrielahlert@gmail.com>.

This is a small project maintained by one person in his spare time. Expect a
first reply within a week. There is no bounty.

## Supported versions

Only the latest release. Given the size of the project, backporting fixes to
older versions is not realistic.

## What StepEasy does that deserves scrutiny

Being upfront about the parts a reviewer should look at first:

- **It installs global input hooks.** While recording, `WH_MOUSE_LL` and
  `WH_KEYBOARD_LL` see every click and keystroke system-wide, including in
  other applications. This is what makes the tool work, and it is also why you
  should not run it while typing passwords.
- **It takes screenshots and stores them.** A recording contains whatever was
  on screen. Blur annotations remove the pixels from the exported image, but
  the **original screenshot stays inside the `.stepeasy` package** — blurring
  is not redaction of the source file. Do not treat a `.stepeasy` as sanitised.
- **It writes autosave drafts to disk.** Unsaved recordings are kept in
  `%APPDATA%\stepeasy\data\recuperacao` so a crash does not lose work. Those
  drafts contain the same screenshots and are only removed when you save or
  discard.

## What it does not do

- No network connection of any kind. No telemetry, no update check, no
  account.
- Nothing leaves the machine unless you export a file and send it yourself.

If you find that any of the above is untrue, that is itself the vulnerability
and we want to hear about it.
