---
name: wealthfolio-cli
description: "Wealthfolio: programmatic access to accounts, activities, net-worth via REST API"
metadata:
  version: 0.2.0
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

### Accounts

```bash
wf accounts list                                 # human table
wf accounts list --json                          # machine — pipe to jq
wf accounts get <id> --json
wf accounts create \
  --name "LINE Bank" --currency TWD \
  --type SAVINGS --tracking TRANSACTIONS
wf accounts update <id> --tracking TRANSACTIONS  # patch a single field
wf accounts delete <id>
```

- `--type` common values: `SAVINGS`, `BROKERAGE`, `CRYPTO`, `CASH`, `ASSET`
- `--tracking`: `HOLDINGS` (snapshot mode — pushed positions) or `TRANSACTIONS` (txn-derived). **Use `TRANSACTIONS` for bank-statement pipelines** — `HOLDINGS` ignores `DEPOSIT`/`WITHDRAWAL` and leaves cash balance at 0.

### Push parsed bank statements

Two flows. Pick by what your pipeline produces.

#### Flow A — CSV (the drag-drop equivalent)

Best for one-off imports, quick debugging, or sourcing CSV from a third
party. Server-side dedup is silent (re-running the same CSV is safe).

```bash
# 1. Cheap server-side parse — confirm the file shape parses
wf activities import-parse --account <id> ./statement.csv

# 2. Full validation (parse + map + check). No write.
wf activities import-check --account <id> ./statement.csv

# 3. Real write
wf activities import --account <id> ./statement.csv
```

CSV columns (case-insensitive, `snake_case` / `camelCase` both accepted):

- **Required:** `date`, `activityType`
- **Cash:** `amount`, `currency`, optional `fee`, `comment`
- **Asset:** `symbol`, `quantity`, `unitPrice`, plus the cash columns
- **Other:** `subtype`, `fxRate`, `id`

Unknown columns are ignored. Dedup is keyed on `(account_id, type, date,
asset_id, qty, price, amount, currency, comment)` — server **ignores**
the `id` column in this flow.

#### Flow B — JSON bulk (the canonical pipeline write path)

Best for parsers that produce typed records with stable bank-side IDs.
Honors `sourceRecordId` in the idempotency hash (so changing `notes`
doesn't break dedup), supports `sourceSystem` tagging, etc.

```bash
wf activities bulk-create ./activities.json
```

The file is either a JSON **array** of `NewActivity` objects:

```json
[
  {
    "accountId": "9b45...",
    "activityType": "WITHDRAWAL",
    "activityDate": "2026-04-01",
    "amount": "2000",
    "currency": "TWD",
    "notes": "[main] transfer",
    "sourceSystem": "LINE_BANK",
    "sourceRecordId": "linebank-1651-2026-04-01-126357",
    "status": "POSTED"
  }
]
```

…or a full **mutation request** (`{creates, updates, deleteIds}`).

**Gotcha:** bulk-create is atomic — a single duplicate fails the whole
batch with HTTP 400. Pre-filter via `wf activities search` first:

```bash
# Skeleton pre-filter pattern
existing=$(wf activities search --account <id> --page-size 500 --json \
    | jq -r '.data[].sourceRecordId // empty' | sort -u)
# Strip rows whose sourceRecordId is in $existing, then bulk-create the rest.
```

### Search / inspect / clean up

```bash
wf activities search --account <id> --page 0 --page-size 50        # 0-indexed!
wf activities search --account <id> --date-from 2026-04-01 --json
wf activities delete <activity-id>
```

### Net-worth snapshot

```bash
wf net-worth current             # human
wf net-worth current --json | jq .netWorth
```

## Symbol normalization (cash activities)

Server clears symbol on `DEPOSIT`/`WITHDRAWAL`/`FEE`/`TAX`/`CREDIT`
regardless of what you pass. Cash-symbol patterns accepted: `$CASH-XXX`,
`CASH:XXX`, `CASH_XXX`, `CASH-XXX` where `XXX` is a 3-letter ISO code.
For pipelines: just send `""`. **Never** send a symbol on
`TRANSFER_IN`/`TRANSFER_OUT` without a quantity — that flags the row for
manual review.

## Output convention

- `stdout` — JSON (with `--json`) or pretty table (default).
- `stderr` — progress, errors, tracing.
- Exit `0` success, non-zero runtime error, `2` clap misuse.

## Error → fix recipes

| Symptom                                                | Likely cause                                                 | Fix                                                              |
| ------------------------------------------------------ | ------------------------------------------------------------ | ---------------------------------------------------------------- |
| `login failed (HTTP 401)`                              | `WEALTHFOLIO_PASSWORD` doesn't match hash                    | Update the configured password                                   |
| `WEALTHFOLIO_BASE_URL not set`                         | env not injected                                             | Set the environment variable                                     |
| `server returned 404` with `accountId`                 | Wrong UUID                                                   | `wf accounts list`                                               |
| `server returned 422` on `accounts create`             | Missing required field                                       | All of name/currency/type/tracking required                      |
| `server returned 400 Duplicate activity detected`      | `bulk-create` hit an already-imported row                    | Pre-filter via `wf activities search` and exclude existing srids |
| `import` finished but `wf net-worth current` is `0`    | Account is `tracking=HOLDINGS`; cash txns don't move balance | `wf accounts update <id> --tracking TRANSACTIONS`                |
| `bulk-create` row missing in net-worth despite success | No opening-balance activity, so account starts at 0          | Add a `DEPOSIT` for the prior period's closing balance           |

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
