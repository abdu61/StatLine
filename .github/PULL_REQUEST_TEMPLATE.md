# Pull request template

## What changed

<!-- One or two sentences. What was broken or missing, and what does this do about it? -->

## Why

<!-- If it fixes an issue, link it. If it is a design decision, say which alternative you rejected. -->

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`
- [ ] `cargo test --all`
- [ ] `cargo deny check`
- [ ] Ran the app and exercised the change by hand

## If this touches the render path or the sampler

- [ ] Measured the cost (`--bench 60`, or `STATLINE_TRACE_TICK=1`) and pasted the numbers
- [ ] Regenerated snapshots and looked at the PNGs
- [ ] No new work happens on a tick where nothing changed

## If this adds a dependency

- [ ] Licence is on the allow-list in `deny.toml`
- [ ] Appended the justification to `docs/adr/0005-dependency-policy.md`
