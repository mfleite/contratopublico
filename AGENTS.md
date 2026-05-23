# Contrato Público — Agent Guide

## Stack
- **Backend:** Rust workspace (3 crates: `api`→`backend` bin, `common`, `scraper`→`cli` bin) + Axum
- **Frontend:** SvelteKit 5 + Tailwind v4 + shadcn-svelte + TypeScript
- **Infra:** Docker Compose, Meilisearch, Postgres 16, rpxy (reverse proxy)
- **Monitoring:** Prometheus + Grafana + k6

## Commands

```sh
# Start infra dependencies (Postgres + Meilisearch)
docker compose -f docker/compose.dev.yml up -d

# Backend (from repo root)
cargo run --release --bin backend          # full (with periodic scraper)
cargo run --release --bin backend -- --no-scraper

# CLI scraper task (manual run inside Docker)
docker compose run --rm cli                # uses --profile manual service

# Frontend
cd frontend && bun install && bun run dev

# Full production-like stack
docker compose -f docker/compose.yml up -d
```

## CI verification commands (run in this order)
```sh
# Backend
DATABASE_URL=postgres://... sqlx migrate run   # required before cargo test
cargo build --verbose && cargo test --verbose

# Frontend
bun install && bun run build                    # no separate lint/typecheck in CI
bun run check                                    # svelte-check (optional locally)
```

## Architecture notes

- **API proxy**: SvelteKit SSR proxies `/api/*` to backend (`BACKEND_URL`). In production, rpxy routes `/api` directly. Client-side fetches go through rpxy, not the SvelteKit proxy.
- **Backend ports**: API on `:3000`, metrics on `:3001/metrics`.
- **Rate limiting**: Burst of 2 per 200ms on `POST /api/search` and `GET /api/contract/{id}`.
- **3 Postgres migrations** in `backend/migrations/` (timestamp-prefixed `.sql`). `.sqlx/` offline data enables compile-time SQL verification without a live DB.
- **Docker build uses `SQLX_OFFLINE=true`** — no Postgres needed at compile time.
- **Frontend adapter**: `svelte-adapter-bun` (not `adapter-auto`). Docker image uses `bun --bun run build`.
- **shadcn-svelte** components under `src/lib/components/ui/`, app components under `src/lib/components/`.
- **Markdown pages** via mdsvex at `/changelog` and `/privacy` (`.md` route files).
- **Clippy/format**: standard Cargo conventions; no custom config.
- **Edition**: Rust 2024.

## Scraping

- The scraper pulls data from `base.gov.pt` (Portugal's official contracts portal).
- Production uses a Warp SOCKS5 proxy (`socks5://warp:1080`) via `BASE_GOV_CLIENT_PROXY` env var (set in `compose.cftunnels.yml`) to avoid IP blocking.
- Scraper state persists to `SAVED_PAGES_PATH` (JSON, mounted volume `/data/scraper`).
- Interval defaults to 3600s; override via `SCRAPER_INTERVAL_SECS`.

## Frontend quirks

- Tailwind v4 with `@tailwindcss/vite` (no `tailwind.config.js` used at runtime; `@config` directive in `app.css` references `tailwind.config.js` only for IntelliSense).
- Dark mode via `mode-watcher` package.
- `engine-strict=true` in `.npmrc` — requires Bun or Node 20+.
- shadcn-svelte alias: `$lib/components/ui` → all UI primitives.
- Utility `cn()` from `$lib/utils.ts` (clsx + tailwind-merge) for class merging.
- Number/currency formatting uses `pt-PT` locale.
