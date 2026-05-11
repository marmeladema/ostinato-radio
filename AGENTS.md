# Ostinato Radio — Agent Notes

## Project Structure

- `src/` — Rust Axum backend
  - `main.rs` — bootstrap, config, server, cache pruning task
  - `config.rs` — TOML + env config loading
  - `errors.rs` — unified AppError + IntoResponse
  - `state.rs` — AppState (Arc), taste profile, sessions, caches
  - `routes/` — HTTP handlers (auth, radio, playback, feedback)
  - `providers/` — third-party clients (Qobuz, Last.fm, AI, LinkPlay/WiiM)
  - `engine/` — pool builder, ranker, window manager, taste profile builder
- `frontend/` — React 18 + TypeScript + Vite PWA
  - `src/components/Home.tsx` — theme input + presets
  - `src/components/RadioSession.tsx` — player + queue + feedback
  - `src/components/Settings.tsx` — WiiM IP, provider info
- `config.toml.example` — configuration template
- `Dockerfile` — multi-stage build with cargo-chef + Node frontend build

## Build & Run

### Backend only (dev)
```bash
. "$HOME/.cargo/env"
cargo run
```

### Frontend (dev)
```bash
cd frontend
npm install
npm run dev
```

### Docker (production)
```bash
docker build -t ostinato-radio .
docker run -p 8080:8080 \
  -e QOBUZ_EMAIL=... \
  -e QOBUZ_PASSWORD=... \
  -e LASTFM_API_KEY=... \
  -e CLOUDFLARE_ACCOUNT_ID=... \
  -e CLOUDFLARE_API_TOKEN=... \
  ostinato-radio
```

## Required Environment Variables

- `QOBUZ_EMAIL` / `QOBUZ_PASSWORD` — Qobuz user credentials
- `LASTFM_API_KEY` — Last.fm API key
- `CLOUDFLARE_ACCOUNT_ID` + `CLOUDFLARE_API_TOKEN` — for Workers AI
- `ANTHROPIC_API_KEY` — optional, for Anthropic AI provider
- `OPENAI_API_KEY` + `OPENAI_BASE_URL` — optional, for OpenAI-compatible provider

## Key Design Decisions

- Pure in-memory state; no database. Restarts rebuild taste profile from Qobuz favorites.
- AI ranking never invents tracks; it only reorders a candidate list from pools.
- Stream URLs are resolved on-demand via 302 redirect to Qobuz CDN.
- WiiM playback uses dynamic M3U playlists with backend redirect URLs.
- CORS is permissive for local development; tighten for production if needed.
