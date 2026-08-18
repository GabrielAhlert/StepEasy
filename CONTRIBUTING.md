# Contributing to StepEasy

Thanks for taking the time. This document is in English because that is what
most contributors read; the application itself speaks Portuguese and English,
and issues in either language are welcome.

## The quickest way to help: translate

StepEasy ships with Brazilian Portuguese and English. Adding a language means
copying one file and translating text — no Rust required. See
**[TRANSLATING.md](TRANSLATING.md)**.

## Building

You need a recent stable Rust toolchain. Nothing else on Windows.

```bash
cargo run --release
```

On Linux you also need the system libraries listed in
`.github/actions/deps-linux/action.yml`. Note that Linux and macOS currently
build and open recordings but **cannot capture** — the input hooks and the
accessibility bridge are Windows-only so far.

To work on the editor without recording anything, generate a synthetic
recording:

```bash
cargo run -p stepeasy-core --example gravacao_exemplo -- exemplo.stepeasy
cargo run -- exemplo.stepeasy
```

## Before opening a pull request

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

CI runs exactly these on Windows and Linux. Nothing else is required.

## How the code is laid out

| Crate | Responsibility |
|---|---|
| `stepeasy-core` | Data model, the `.stepeasy` package, caption generation, editing with undo, exporters |
| `stepeasy-capture` | Input hooks, screen capture, accessibility — one implementation per platform |
| `stepeasy-ui` | The egui interface |
| `stepeasy` | The binary |

Two rules that keep this working:

- **`stepeasy-core` knows nothing about platform or interface.** That is what
  lets the whole editing and file logic be tested without opening a window.
- **Anything that needs a real screen goes behind a trait** in
  `stepeasy-capture`, so the logic around it can be tested with invented
  monitors instead of real ones.

## Tests

Cover the logic, not the plumbing. A test that reimplements the code it is
testing is worse than no test — it passes while the real thing rots. If
testing something requires duplicating its rules, extract the rules into a
function the test can call.

Platform code (`stepeasy-capture/src/windows/`) has no automated tests; it is
`unsafe` FFI that only real hardware exercises. Say in your pull request what
you tested by hand.

## Commits

Written in the imperative, explaining **why** rather than what. The diff
already shows what changed. Prefixes in use: `feat:`, `fix:`, `docs:`, `ci:`,
`refactor:`, `test:`, `chore:`.

## Reporting bugs

Use the issue templates. For a capture bug, the Windows version, display
scaling and the capture scope you used matter more than anything else — most
capture problems come from multi-monitor setups or DPI scaling.

## Security

Do not open a public issue for a vulnerability. See [SECURITY.md](SECURITY.md).
