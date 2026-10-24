# Third-party notices

Statline itself is MIT-licensed; see [`LICENSE`](LICENSE).

The shipped binary links the following crates. All of them use permissive licences
that `deny.toml` enforces in CI.

| Crate | Licence | Copyright |
|---|---|---|
| `windows` / `windows-core` / `windows-*` | MIT OR Apache-2.0 | © Microsoft Corporation |
| `log` | MIT OR Apache-2.0 | © The Rust Project Developers |
| `thiserror` / `thiserror-impl` | MIT OR Apache-2.0 | © David Tolnay |
| `serde` / `serde_derive` / `serde_json` | MIT OR Apache-2.0 | © Erick Tryzelaar, David Tolnay and contributors |
| `arc-swap` | MIT OR Apache-2.0 | © Michal Vaner |

Transitive dependencies are pinned in `Cargo.lock` and restricted to the same permissive
set by [`deny.toml`](deny.toml): MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, ISC, Zlib,
Unicode-3.0 and 0BSD. No GPL, LGPL, AGPL or SSPL code is linked.

Full licence texts for the two common cases ship in [`licenses/`](licenses/):

- [`licenses/MIT.txt`](licenses/MIT.txt)
- [`licenses/Apache-2.0.txt`](licenses/Apache-2.0.txt)

Windows API names, structures and constants are used under the terms of the Windows SDK;
no SDK source code is redistributed.
