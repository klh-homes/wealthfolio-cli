---
name: wealthfolio-cli
description: "Wealthfolio: programmatic access to accounts, activities, net-worth via REST API"
metadata:
  version: 0.1.0
  category: "finance"
  requires:
    bins:
      - wf
---

# `wf` — Wealthfolio CLI

Single-binary Rust tool. Use this when an agent or shell script needs to
push parsed bank/broker statements into a Wealthfolio
instance, or read account/net-worth state.

## Authentication

Two env vars configure the target Wealthfolio instance:

| Env                    | Purpose                                                      |
| ---------------------- | ------------------------------------------------------------ |
| `WEALTHFOLIO_BASE_URL` | e.g. `https://wealthfolio.example.com`                       |
| `WEALTHFOLIO_PASSWORD` | Plaintext matching server's Argon2id `WF_AUTH_PASSWORD_HASH` |

Session cookie cached at `~/.cache/wf/cookies.json` (mode 0600). Auto
refreshes when ≥50 % of the 60-min TTL has elapsed. You should
**never** read or pass this file around; just let `wf` manage it.

## Common patterns

### Sanity check before doing anything

```bash
wf doctor
```

Reports env, connectivity, login round-trip, and an authenticated call.
Run this first when anything misbehaves.

### List accounts

```bash
wf accounts list                 # human table
wf accounts list --json          # machine — pipe to jq
```

### Create an account

```bash
wf accounts create \
  --name "Checking Account" \
  --currency TWD \
  --type SAVINGS \
  --tracking HOLDINGS
```

- `--type` common values: `SAVINGS`, `BROKERAGE`, `CRYPTO`, `CASH`, `ASSET`
- `--tracking`: `HOLDINGS` (periodic balance snapshots) or `TRANSACTIONS` (full activity ledger)
- Optional: `--default`, `--platform-id`, `--group`, `--account-number`

### Push parsed bank statement (the main pipeline write path)

```bash
# 1. Sanity check the CSV without writing anything
wf activities import-parse --account <id> ./statement.csv

# 2. Full validation (parse + check) without writing
wf activities import-check --account <id> ./statement.csv

# 3. Real write
wf activities import --account <id> ./statement.csv
```

All three use `multipart/form-data` (same code path as the web UI's
drag-drop). CSV format reference: see Wealthfolio's
`docs/activities/activity-types.md` upstream — the 14 canonical
activity types (`BUY`, `SELL`, `DEPOSIT`, `WITHDRAWAL`, `DIVIDEND`,
`ADJUSTMENT`, etc.) and required columns per type.

### Net-worth snapshot

```bash
wf net-worth current             # human
wf net-worth current --json | jq .netWorth
```

## Output convention

- `stdout` — JSON (with `--json`) or pretty table (default).
- `stderr` — progress, errors, tracing.
- Exit `0` success, `1` runtime error, `2` clap misuse.

## Error → fix recipes

| Symptom                                    | Likely cause                              | Fix                                         |
| ------------------------------------------ | ----------------------------------------- | ------------------------------------------- |
| `login failed (HTTP 401)`                  | `WEALTHFOLIO_PASSWORD` doesn't match hash | Update the configured password              |
| `WEALTHFOLIO_BASE_URL not set`             | env not injected                          | Set the environment variable                |
| `server returned 404` with `accountId`     | Wrong UUID                                | `wf accounts list` to find right ID         |
| `server returned 422` on `accounts create` | Missing required field                    | All of name/currency/type/tracking required |

## Verbose mode for debugging

```bash
wf -v <command>      # info-level
wf -vv <command>     # debug-level (HTTP traces)
RUST_LOG=trace wf <command>   # full trace
```

## Versioning

`wf --version` reports the bundled crate version. Downstream Dockerfiles
can pin the image tag or copy `/usr/local/bin/wf` and `/skills` from the
published container image.
