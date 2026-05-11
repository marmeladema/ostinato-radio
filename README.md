# Ostinato Radio

> *Ostinato: a musical motif persistently repeated throughout a composition.*
> **Ostinato Radio** is a self-hosted, personalized, infinite radio generator built on top of **Qobuz**, driven by any musical theme — a mood ("chill"), a genre ("folk"), an ambiance ("soir d'hiver"), or an occasion ("running").

Play seamlessly on your **phone** (via a PWA) or on a **WiiM Ultra** streamer on your local network.

---

## Why Ostinato?

Qobuz has excellent audio quality but its editorial playlists are static and impersonal. Streaming devices like the WiiM Ultra are powerful but remote control options are limited. Ostinato Radio bridges that gap: it builds a dynamic, ever-evolving queue from your Qobuz favorites, new releases, and discovery artists — all ranked by an AI to match the theme you chose.

- No third-party scrobbling required — personalization comes from your Qobuz favorites
- No database — everything runs in memory; restarts rebuild the taste profile in seconds
- No track invention — the AI only reorders candidate tracks, never hallucinates music

---

## Features

- **Theme-driven infinite radio** — free text or one-tap presets
- **Three blended pools** per radio:
  - **Familiar** (~60%) — artists and tracks from your Qobuz favorites
  - **New Releases** (~25%) — recent albums from favorite artists
  - **Discovery** (~15%) — similar artists via Last.fm, resolved to Qobuz tracks
- **AI-powered ranking** — configurable provider (Workers AI, Anthropic, OpenAI-compatible) with a deterministic fallback
- **Phone playback** — HTML5 audio in the PWA with skip/complete feedback
- **WiiM playback** — dynamic M3U playlists sent via LinkPlay HTTP API
- **Implicit feedback** — skip/completion rates refine the current session's recommendations in memory
- **Installable PWA** — add to your Android home screen; works offline as a shell

---

## Architecture

```
┌──────────────────────────────────────┐
│  PWA (React 18 + Vite + TypeScript)  │
│  ─────────────────────────────────   │
│  - Theme input + presets             │
│  - Radio queue + player              │
│  - Skip / Complete controls          │
│  - "Play on WiiM" toggle             │
└──────────────┬───────────────────────┘
               │ HTTPS
               ▼
┌──────────────────────────────────────┐
│  Backend (Rust + Axum + Tokio)       │
│  ─────────────────────────────────   │
│  - Qobuz API client + auth scraper   │
│  - Last.fm API client                │
│  - AI provider abstraction           │
│  - Pool builder + ranker + window    │
│  - In-memory state (no database!)    │
│  - Stream redirect (302 to Qobuz)    │
│  - M3U generator + LinkPlay client   │
└──────┬────────────┬────────────┬────┘
       │            │            │
       ▼            ▼            ▼
   Qobuz API   Last.fm API   AI Provider
                                 │
                                 ▼
                            WiiM Ultra
```

---

## Quick Start

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable)
- [Node.js](https://nodejs.org/) 20+ (for frontend dev only)
- A **Qobuz** account
- A **Last.fm API key** ([get one here](https://www.last.fm/api/account/create))
- (Optional) **Cloudflare Workers AI** credentials for AI ranking

### 1. Clone & configure

```bash
git clone https://github.com/yourusername/ostinato-radio.git
cd ostinato-radio

# Copy the example config
cp config.toml.example config.toml
# Edit config.toml to match your network (especially public_base_url and wiim ip)
```

### 2. Set environment variables

```bash
export QOBUZ_EMAIL="your@email.com"
export QOBUZ_PASSWORD="your_qobuz_password"
export LASTFM_API_KEY="your_lastfm_api_key"

# Optional: for AI ranking instead of deterministic fallback
export CLOUDFLARE_ACCOUNT_ID="..."
export CLOUDFLARE_API_TOKEN="..."
```

### 3. Run the backend

```bash
cargo run
```

The server will:
1. Scrape Qobuz's `bundle.js` for app credentials
2. Log in with your email/password
3. Fetch your favorites and build the taste profile
4. Start serving on `0.0.0.0:8080`

### 4. Run the frontend (dev)

```bash
cd frontend
npm install
npm run dev
```

Vite dev server proxies API requests to `localhost:8080`. Open the local URL it prints (usually `http://localhost:5173`).

### 5. Install as PWA (phone)

Open the frontend in Chrome on Android → Menu → "Add to Home screen". The app will work as a fullscreen PWA.

---

## Docker (Production)

Build and run everything in a single container:

```bash
docker build -t ostinato-radio .

docker run -p 8080:8080 \
  -e QOBUZ_EMAIL=your@email.com \
  -e QOBUZ_PASSWORD=yourpass \
  -e LASTFM_API_KEY=yourkey \
  -e CLOUDFLARE_ACCOUNT_ID=... \
  -e CLOUDFLARE_API_TOKEN=... \
  ostinato-radio
```

The backend serves the built frontend statically from `/`.

---

## Configuration

`config.toml` (or via env vars with prefix `OSTINATO__`):

```toml
[server]
host = "0.0.0.0"
port = 8080
public_base_url = "http://192.168.1.10:8080"  # must be reachable from WiiM

[qobuz]
preferred_format_id = 27  # Hi-Res 24-bit

[lastfm]
api_key = "your_lastfm_api_key"

[ai]
provider = "workers_ai"  # or "anthropic" | "openai_compat"
model = "@cf/google/gemma-4-26b-a4b-it"

[radio]
default_pool_ratios = { familiar = 0.60, new_release = 0.25, discovery = 0.15 }
window_size = 20
window_refresh_threshold = 5
new_release_max_age_days = 180

[wiim]
ip = "192.168.1.42"       # optional: auto-discovery not yet implemented
poll_interval_seconds = 5
```

### Environment Variables Reference

| Variable | Required? | Purpose |
|---|---|---|
| `QOBUZ_EMAIL` | Yes | Qobuz login email |
| `QOBUZ_PASSWORD` | Yes | Qobuz login password |
| `QOBUZ_APP_ID` | No | Manual override for Qobuz app_id (if bundle scraping breaks) |
| `QOBUZ_APP_SECRET` | No | Manual override for Qobuz app_secret |
| `LASTFM_API_KEY` | Yes | Last.fm API key |
| `CLOUDFLARE_ACCOUNT_ID` | No | For Workers AI |
| `CLOUDFLARE_API_TOKEN` | No | For Workers AI |
| `ANTHROPIC_API_KEY` | No | For Anthropic AI provider |
| `OPENAI_API_KEY` | No | For OpenAI-compatible provider |
| `OPENAI_BASE_URL` | No | Base URL for OpenAI-compatible gateway |

---

## API Overview

| Endpoint | Method | Description |
|---|---|---|
| `/health` | GET | Health check |
| `/auth/status` | GET | Qobuz auth status |
| `/radio/start` | POST | Start a new radio session |
| `/radio/{session_id}` | GET | Session status + current track |
| `/radio/{session_id}/next` | POST | Advance to next track |
| `/stream/{track_id}` | GET | Stream redirect (302 to Qobuz CDN) |
| `/playback/wiim/{session_id}.m3u` | GET | Dynamic M3U playlist |
| `/playback/control?command={cmd}` | GET | WiiM control (pause/resume/stop/next/prev/vol:N) |
| `/feedback/{session_id}` | POST | Submit skip/complete/progress feedback |

---

## Project Structure

```
.
├── src/                          # Rust backend
│   ├── main.rs                   # Bootstrap, server, cache pruning task
│   ├── config.rs                 # TOML + env config loading
│   ├── errors.rs                 # Unified AppError
│   ├── state.rs                  # AppState, TasteProfile, caches, sessions
│   ├── routes/                   # HTTP handlers
│   │   ├── auth.rs
│   │   ├── radio.rs
│   │   ├── playback.rs
│   │   └── feedback.rs
│   ├── providers/                # Third-party API clients
│   │   ├── qobuz/                # Auth scraper + API client
│   │   ├── lastfm/
│   │   ├── ai/                   # Trait + Workers AI / Anthropic / OpenAI
│   │   └── linkplay/             # WiiM HTTP control
│   └── engine/                   # Radio engine
│       ├── pools.rs              # Build 3 candidate pools
│       ├── ranker.rs             # AI ranking
│       ├── window.rs             # Sliding window queue management
│       └── profile.rs            # Taste profile derivation
├── frontend/                     # React 18 + Vite PWA
│   ├── src/
│   │   ├── App.tsx
│   │   ├── api.ts                # Backend API client
│   │   ├── hooks/useAudio.ts     # HTML5 audio hook
│   │   └── components/
│   │       ├── Home.tsx          # Theme input + presets
│   │       ├── RadioSession.tsx  # Player + queue
│   │       └── Settings.tsx      # WiiM IP, provider info
│   └── public/
│       ├── manifest.json
│       └── sw.js                 # Service worker
├── config.toml.example
├── Dockerfile                    # Multi-stage production build
├── AGENTS.md                     # Agent-focused notes
├── KNOWN_ISSUES.md               # Gaps, stubs, empirical validations
└── ostinato-radio-spec.md        # Full implementation specification
```

---

## Development

### Backend only

```bash
# Check compilation
cargo check

# Linting
cargo clippy

# Formatting
cargo fmt

# Run with debug logs
RUST_LOG=debug cargo run
```

### Frontend only

```bash
cd frontend
npm run dev
```

### Build release binary

```bash
cargo build --release
# Binary: target/release/ostinato-radio
```

---

## Key Design Decisions

- **Pure in-memory state** — no database. Restarts rebuild the taste profile from Qobuz favorites in seconds. Single-user, self-hosted by design.
- **AI never invents tracks** — it only reorders a candidate list assembled by the pool builder. This keeps smaller open-weight models reliable.
- **On-demand streams** — each request to `/stream/{track_id}` triggers a fresh `getFileUrl` call and returns a 302 redirect. No stream URL caching.
- **WiiM via M3U** — the backend generates dynamic playlists with backend redirect URLs, so the WiiM can play transparently through the Qobuz CDN.
- **CORS is permissive** for local development; lock it down for production if needed.

---

## Known Limitations & Roadmap

See [`KNOWN_ISSUES.md`](./KNOWN_ISSUES.md) for a detailed breakdown of:
- What's implemented vs. stubbed
- Open empirical validations (e.g. WiiM M3U behavior, Qobuz IP restrictions)
- Missing features (sliding window auto-refresh, Anthropic/OpenAI provider implementations, WiiM auto-discovery)

High-level next steps:
1. Test dynamic M3U + 302 redirect behavior on real WiiM hardware
2. Implement Anthropic and OpenAI-compatible AI providers
3. Wire automatic sliding-window refresh when queue runs low
4. Fetch individual album tracks for the New Releases pool
5. Add tests for Qobuz signing, pool builder, and ranker fallback

---

## License

MIT

---

> Built with Rust, Axum, React, and a lot of music.
