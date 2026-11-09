# Contributing to StatLine

Thanks for taking a look. This is a small project with a strict budget, so a few rules
matter more than usual.

## Before you start

Read [`AGENTS.md`](AGENTS.md). It is the operational summary: the resource budget, the
`unsafe` policy, the dependency rules and the verification gate. Then read the one spec
that covers the area you are touching under [`docs/`](docs/INDEX.md) — do not read the
whole tree.

## The gate

Every pull request must pass the same four checks CI runs:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo deny check
```

If you change anything visual, also generate and inspect the snapshots:

```powershell
cargo run --release -p statline-app -- --render-snapshots .\snapshots-out
```

Read the PNGs. Do not claim a UI change works without looking at pixels.

## House rules

- **Resource budget is a feature.** Idle cost, working set and handle count are part of
  the product. A change that regresses one needs a measurement and a note, not a quiet
  pass.
- **`unsafe` lives in `statline-win` and the Direct2D calls in `statline-render`.** Every
  block gets a `// SAFETY:` comment naming the invariant that makes it sound.
- **No panics in production paths.** `unwrap`/`expect` are for tests and genuinely
  unreachable invariants, with a comment.
- **New dependencies need justification** — licence, size, why. Append to
  `docs/adr/0005-dependency-policy.md`.
- **No code copied from other monitor projects.** Win32 usage is written from the public
  API documentation. Behavioural references are fine; their source is not.

## Commits

Conventional commits, one logical change each:

```
feat(metrics): add per-core CPU sampling
fix(taskbar): keep strip aligned after DPI change
docs(status): mark M3 complete
```

## Reporting a bug

Include your Windows build (`winver`), the output of `statline.exe --probe`, and the tail
of `%LOCALAPPDATA%\StatLine\logs\statline.log`. If it is visual, a screenshot of the
strip or panel helps more than a description.
