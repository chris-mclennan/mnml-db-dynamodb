# Contributing to mnml-db-docdb

Thanks for taking a look! This repo is part of the [mnml integration family](https://mnml.sh/manual/integrations/community/) — a standalone Amazon DocumentDB / MongoDB viewer that doubles as a hosted mnml pane.

## Two paths

**A. You want to fix a bug or add a DocumentDB/MongoDB-specific feature here.** Open an issue or PR against this repo. See "Local development" below.

**B. You want a viewer for a different backend.** **Fork this repo** and replace `src/docdb.rs` with your backend. The rest of the scaffold (`blit.rs`, `config.rs`, `ui.rs`, `keys.rs`, `app.rs`) is designed to be copy-pasted. See [Building integrations](https://mnml.sh/manual/integrations/building/) for the full guide. You don't owe anything back to this repo or to mnml — your fork can live under your own name.

## Project layout

```
src/
├── main.rs                # CLI + mode dispatch (TUI / --blit / --check)
├── app.rs                 # state — connections, query buffer, results
├── config.rs              # ~/.config/mnml-db-docdb.toml
├── docdb.rs               # ← the backend-specific file (swap this when forking)
├── keys.rs                # action enum + key bindings
├── ui.rs                  # ratatui draw + crossterm loop
└── blit.rs                # tmnl-protocol over UDS — copied verbatim
```

This one is a good fork target for **document-store backends** (CouchDB, Firestore, an internal document API). The "query" is parsed as a `{...}` filter and results render as `_id` + JSON-stringified document — that two-column shape carries to most document stores.

`blit.rs` is shared verbatim across the family. Patches to `blit.rs` should land first in [`mnml-db-postgres`](https://github.com/chris-mclennan/mnml-db-postgres) and then be ported to the siblings.

## Local development

```sh
git clone https://github.com/chris-mclennan/mnml-db-docdb
cd mnml-db-docdb
cargo build
cargo test
cargo clippy --all-targets        # must be warning-free
cargo fmt                          # before committing
```

Spin up a local MongoDB for manual testing (DocumentDB is MongoDB-wire-compatible, so this exercises the same paths):

```sh
docker run -d --name mongo-mnml -p 27017:27017 mongo:7
cargo run -- --check
cargo run
```

## PR conventions

- One commit per logical change is fine; squash on merge is fine too.
- Commit messages: short imperative subject (≤72 chars), optional body explaining "why".
- Add a unit test for any backend behavior you change (`src/docdb.rs` has parser tests).
- `cargo clippy --all-targets` and `cargo fmt --check` must be clean.

## License + ownership

MIT. Contributions are accepted under the same license. No copyright assignment required; you keep authorship of your changes.
