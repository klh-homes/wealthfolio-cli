# wealthfolio-cli

Rust CLI for [Wealthfolio's](https://wealthfolio.app) self-hosted REST API. Designed for AI agents and shell scripts that need to push parsed bank/broker statements into a Wealthfolio instance — `wf` handles auth (cookie cache + auto-refresh), account ops, CSV import, and net-worth queries.

## Why this exists

Wealthfolio is single-user with cookie-based JWT auth and **no Personal Access Token**. Every external integration has to log in with the same password the human uses, manage the 60-minute session, and speak the same multipart CSV format the web UI's drag-drop import speaks. `wf` is a thin, typed wrapper around those calls so automation scripts and humans debugging them can stay focused on the parsing logic.

## Install

### From the container image (recommended for COPY --from)

```bash
docker pull ghcr.io/klh-homes/wealthfolio-cli:latest
```

Tagged image (e.g. `ghcr.io/klh-homes/wealthfolio-cli:v0.1.0`). The image bundles the `wf` binary at `/usr/local/bin/wf` and the `SKILL.md` under `/skills/wealthfolio-cli/SKILL.md` for downstream Dockerfiles to pick up with `COPY --from=...`.

### From cargo

```bash
cargo install --git https://github.com/klh-homes/wealthfolio-cli --tag v0.1.0
```

### From source

```bash
git clone https://github.com/klh-homes/wealthfolio-cli && cd wealthfolio-cli
cargo build --release
./target/release/wf --version
```

## Configure

Two env vars configure the target Wealthfolio instance:

```bash
export WEALTHFOLIO_BASE_URL=https://wealthfolio.example.com
export WEALTHFOLIO_PASSWORD='…'   # plaintext that matches WF_AUTH_PASSWORD_HASH
```

Session cookies live at `~/.cache/wf/cookies.json` (XDG-compliant, mode 0600).

## Commands

```text
wf doctor                                        # env + DNS + login health
wf login | logout                                # cached session mgmt (auto)
wf accounts list [--json]
wf accounts get <id>
wf accounts create --name X --currency TWD --type SAVINGS --tracking HOLDINGS
wf accounts delete <id>
wf activities import-parse  --account <id> file.csv   # dry-run (no write)
wf activities import-check  --account <id> file.csv   # full validation
wf activities import        --account <id> file.csv   # actual write
wf net-worth current [--json]
```

See [`skills/wealthfolio-cli/SKILL.md`](skills/wealthfolio-cli/SKILL.md) for AI-agent usage patterns + error-recipe table.

## License

MIT
