# mnml-db-dynamodb

Amazon DynamoDB table browser for [mnml](https://mnml.sh) —
terminal TUI for scanning tables and viewing items. Runs
standalone in any terminal or as a hosted mnml pane. Shells
out to the `aws` CLI; no AWS SDK dep.

```
┌─ dynamodb ───────────────────────────────────────────────────────┐
│ ▸1.Sessions (47)  2.Orders (50)  3.Events                        │
└──────────────────────────────────────────────────────────────────┘
┌─ Sessions · pk: userId / ts ─┐ ┌─ focused item ────────────────┐ │
│ ▸ user-abc · 1717685623      │ │ {                              │ │
│   user-abc · 1717685420      │ │   "userId": { "S": "abc" },    │ │
│   user-xyz · 1717684901      │ │   "ts": { "N": "1717685623" }, │ │
│   user-xyz · 1717684500      │ │   "device": { "S": "iOS" },    │ │
│   …                          │ │   "appVer": { "S": "4.2.1" }   │ │
│                              │ │ }                              │ │
└──────────────────────────────┘ └────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────┘
  1-9 tab · ↑↓/jk move · o console · y yank JSON · r refresh · q quit
```

## Install

```sh
cargo install --git https://github.com/chris-mclennan/mnml-db-dynamodb mnml-db-dynamodb
```

You'll also need the [AWS CLI](https://aws.amazon.com/cli/) on
your `$PATH` with credentials.

## Setup

1. Verify the AWS CLI works: `aws dynamodb list-tables`.
2. Run once to scaffold the config: `mnml-db-dynamodb`.
3. Edit `~/.config/mnml-db-dynamodb.toml`.
4. Re-run.

## Config

```toml
# Optional top-level region:
# region = "us-east-1"

[[tabs]]
name = "Sessions"
table = "user-sessions"
scan_limit = 50

[[tabs]]
name = "Orders"
table = "orders"
scan_limit = 100
```

Each `[[tabs]]` is one table. `scan_limit` caps `aws dynamodb scan`
at N items per refresh (default 50, max 1000).

## Auth shape

There is none on this viewer's side. Every operation is
`aws dynamodb scan` / `describe-table` as a subprocess. The CLI's
credential chain authenticates. Same shape as the other AWS
siblings ([`mnml-aws-codebuild`](https://github.com/chris-mclennan/mnml-aws-codebuild),
[`mnml-aws-cloudwatch-logs`](https://github.com/chris-mclennan/mnml-aws-cloudwatch-logs),
[`mnml-aws-amplify`](https://github.com/chris-mclennan/mnml-aws-amplify))
— if one works, the others will.

## Keys

| Chord | Action |
|---|---|
| `1`-`9` | Switch to that tab |
| `Tab` / `BackTab` | Cycle tabs |
| `↑` / `k`, `↓` / `j` | Move selection |
| `o` | Open DynamoDB console for the active table |
| `y` | Yank focused item's pretty-printed JSON to OS clipboard |
| `r` | Refresh active tab (re-scan) |
| `q` / `Esc` / `Ctrl+C` | Quit |

## Layout

- **Tab strip:** tabs + per-tab item count.
- **Items table (left):** primary key + sort key as the first
  column, compact summary of remaining fields as the second.
- **Detail panel (right):** focused item's full JSON,
  pretty-printed. Same JSON `y` yanks.
- **Status:** active table, scan count, key hints.

Primary key resolution uses `describe-table` to find the
hash/range key fields, so the "PRIMARY" column shows whatever's
right for your table (`userId`, `pk`, `sessionId`, etc.) without
config.

## Status

**v0.1 (this release)** — `scan`-based table browsing,
focused-item JSON panel, console open, item JSON yank. Standalone
TUI + `--blit` host-pane mode.

Held back for v0.2+:
- `query` instead of `scan` (partition-key-anchored lookups)
- Filter expression input (`FilterExpression`)
- Pagination — currently first scan_limit items only
- Item editing
- GSI / LSI browsing
- Stream tail (DynamoDB Streams → live item changes)

## License

MIT.
