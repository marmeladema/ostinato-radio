# ostinato-radio — Implementation Spec

> *Ostinato: a musical motif persistently repeated throughout a composition. Here: a personalized, infinite radio over Qobuz.*

## 1. Context & Goals

Build a self-hosted PWA + backend (`ostinato-radio`) that generates **personalized, infinite radios** on top of Qobuz, driven by any musical theme — a mood ("chill"), a genre ("folk"), an ambiance ("soir d'hiver"), or an occasion ("running") — with seamless playback on both an Android phone and a WiiM Ultra streamer on the local network.

The system fills the gap left by:
- Qobuz's static, non-personalized editorial playlists (Mood, Genre, etc.)
- Deezer Connect (discontinued May 2025), which previously enabled remote control of streamers
- Deezer's closed API (no new tokens issued)

### Functional requirements

- Infinite radio queue driven by a **musical theme** — mood, genre, ambiance, or occasion (free text or presets)
- Personalization based on the user's Qobuz favorites (no third-party scrobbling required)
- Three blended pools per radio:
  - **Familiar** — known artists/tracks (~60% by default)
  - **New releases from known artists** (~25%)
  - **Discovery** — new similar artists (~15%)
- One-tap playback on phone (built-in player) or on WiiM Ultra (via LinkPlay)
- Implicit feedback (skip rate, completion rate) refines future recommendations
- AI provider is configurable (Workers AI, Anthropic, OpenAI-compatible)

### Non-goals

- Replacing the Qobuz native app for casual browsing
- Supporting multiple users in a multi-tenant fashion (single-user, self-hosted)
- Audio file download / offline playback
- DRM bypass or any redistribution

---

## 2. Architecture Overview

```
┌─────────────────────────────────────────────────────┐
│  PWA (Android-installable)                          │
│  ───────────────────────────────────────────────    │
│  - Theme input (free text + presets)                │
│  - Radio queue UI                                   │
│  - HTML5 Audio player (phone playback)              │
│  - "Play on WiiM" button                            │
│  - Implicit feedback capture (skip/complete)        │
└──────────────────────────┬──────────────────────────┘
                           │ HTTPS (local domain)
                           ▼
┌─────────────────────────────────────────────────────┐
│  Backend (Axum / Rust, Docker)                      │
│  ───────────────────────────────────────────────    │
│  - Qobuz API client (bundle scraping + auth)        │
│  - Last.fm API client                               │
│  - AI provider abstraction (trait + impls)          │
│  - Radio engine (pool builder + ranking + window)   │
│  - In-memory state (taste profile, caches, sessions)│
│  - Stream redirect endpoint (signed URL refresh)    │
│  - M3U playlist generator                           │
│  - LinkPlay HTTP client (WiiM control)              │
└──────┬────────────┬────────────┬────────────┬───────┘
       │            │            │            │
       ▼            ▼            ▼            ▼
   Qobuz API   Last.fm API   AI Provider   WiiM Ultra
                                          (LinkPlay HTTP)
```

The backend is the single source of truth. The PWA is a thin client that calls backend endpoints; all third-party API credentials live server-side.

---

## 3. Components

### 3.1 Backend (Rust / Axum)

**Tech stack**
- `axum` for HTTP routing
- `tokio` async runtime + `tokio::sync::RwLock` for shared state
- `reqwest` HTTP client
- `serde` for JSON
- `dashmap` or `parking_lot` for concurrent in-memory maps where useful
- `tracing` for structured logs

**State model: pure in-memory.** No database. All state lives in a single `AppState` struct shared across handlers via `Arc`. State is rebuilt at boot from Qobuz favorites and external APIs. Restarts are cheap (a few seconds) and acceptable for this single-user use case.

**Layout**
```
src/
  main.rs                  # bootstrap, config, axum server
  config.rs                # config loading (env + TOML)
  errors.rs                # unified error type
  routes/
    auth.rs                # Qobuz login / token mgmt
    radio.rs               # generate / next / current
    playback.rs            # stream redirect, m3u, WiiM control
    feedback.rs            # skip/complete signals
  providers/
    qobuz/                 # API client + bundle scraper
    lastfm/                # API client
    ai/
      mod.rs               # MusicAI trait
      workers_ai.rs        # Cloudflare impl
      anthropic.rs         # Anthropic impl
      openai_compat.rs     # OpenAI-compatible impl
    linkplay/              # WiiM control
  engine/
    pools.rs               # build the three candidate pools
    ranker.rs              # AI-based ranking
    window.rs              # sliding window mgmt
    profile.rs             # taste profile derivation
  state.rs                 # AppState (in-memory shared state)
```

### 3.2 Qobuz integration

**Authentication**
- App credentials (`app_id`, `app_secret`) extracted at startup by scraping `play.qobuz.com`'s `bundle.js`. Refresh on signature failure.
- User authentication: email + password → `user/login` → `user_auth_token` (stored encrypted at rest, refreshed only on revocation).

**Key endpoints used**
- `user/login` — initial auth
- `favorite/getUserFavorites` — load taste profile (artists, albums, tracks)
- `artist/get?artist_id=...&extra=albums` — recent releases per artist (sorted by `release_date`, descending)
- `catalog/search?type=tracks&query=...` — resolve candidate (artist, title) → Qobuz track ID
- `track/get?track_id=...` — track metadata
- `playlist/create`, `playlist/addTracks` — only if mirror-to-Qobuz mode is enabled (optional)
- `track/getFileUrl?track_id=...&format_id=27&request_ts=...&request_sig=...` — signed stream URL

**Signature**
`request_sig = MD5(object + method + sorted_params_concat + request_ts + app_secret)`
(excluding `app_id` and `user_auth_token`).

**Stream URL handling**
- URLs returned by `getFileUrl` are time-limited but **not IP-bound** (to be verified empirically before relying on; see §10).
- The backend never caches stream URLs. Each request to `/stream/{track_id}` triggers a fresh `getFileUrl` call and returns a `302` redirect to the signed CDN URL.

### 3.3 Last.fm integration

API key only (no user account, no scrobbling). Endpoints:
- `artist.getSimilar` — for discovery pool
- `artist.getTopTracks` — candidate tracks per artist
- `tag.getTopTracks` — candidates by theme tag
- `track.getInfo` — tag retrieval for filtering

Map theme input → Last.fm tags via a hand-curated dictionary (e.g. "chill folk soir" → `[chill, folk, acoustic, mellow]`; "running" → `[energetic, electronic, upbeat]`; "shoegaze" → `[shoegaze, dream-pop, noise-pop]`), expanded via `tag.getSimilar` if needed. The dictionary covers moods, genres, ambiances, and occasions uniformly.

### 3.4 AI provider abstraction

```rust
#[async_trait]
pub trait MusicAI: Send + Sync {
    async fn rank_candidates(
        &self,
        ctx: &RankingContext,
    ) -> Result<Vec<RankedTrack>>;
}

pub struct RankingContext<'a> {
    pub theme: &'a str,
    pub taste_profile: &'a TasteProfile,
    pub candidates: &'a [Candidate], // pool-tagged
    pub already_played: &'a [TrackId],
    pub target_count: usize,
    pub pool_ratios: PoolRatios,
}
```

**Implementations**
- `WorkersAIProvider`: POST to `https://api.cloudflare.com/client/v4/accounts/{id}/ai/run/{model}`. Default model: `@cf/google/gemma-4-26b-a4b-it`.
- `AnthropicProvider`: standard Messages API.
- `OpenAICompatProvider`: configurable `base_url` — covers OpenAI, Mistral La Plateforme, and most OSS gateways.

**Prompt structure** (model-agnostic)
- System: defines the task as ranking a candidate list, never generating tracks, and outputting strict JSON.
- User: theme + compact taste profile + JSON candidate list with pool tag + already-played IDs + ratio targets.
- Response format: JSON array of `{candidate_id, rank}` only. Validate strictly; on parse failure, retry once with a stricter prompt, then fall back to a deterministic ranker.

The AI **never invents tracks**. Its only input space is the candidate list. This is what allows smaller open-weight models to perform well here.

### 3.5 LinkPlay / WiiM client

The WiiM Ultra exposes a local HTTP API on `http://<wiim-ip>/httpapi.asp?command=...`.

Discovery: SSDP / mDNS scan on first run, IP cached. Manual override in config.

Commands used:
- `setPlayerCmd:play:<url>` — play a single URL or M3U
- `setPlayerCmd:pause` / `:resume` / `:stop` / `:next` / `:prev`
- `setPlayerCmd:vol:<n>`
- `getPlayerStatus` — poll for current position / state

**Playback flow on WiiM**
1. Backend generates a dynamic M3U at `/playback/wiim/{session_id}.m3u`
2. Each line is `http://<backend-local-ip>:<port>/stream/{track_id}?session={session_id}`
3. Backend sends `setPlayerCmd:play:<m3u-url>` to the WiiM
4. WiiM fetches M3U, then each track URL → backend redirects (302) to a fresh Qobuz signed URL
5. Backend polls `getPlayerStatus` every 5s to track position and trigger sliding-window refresh when needed

The backend must be reachable from the WiiM on the local network — bind to `0.0.0.0`, not `127.0.0.1`, and configure Docker port mapping accordingly.

### 3.6 PWA frontend

**Tech**
- React 18 + TypeScript + Vite
- Service worker for installability and offline shell (not offline playback)
- `<audio>` element for phone playback, controlled via a small player hook
- No state management library needed; `useState` + `useReducer` + SWR for data fetching

**Views**
1. Auth screen (Qobuz login, once)
2. Home: theme input + preset buttons + recent radios
3. Radio session: current track, queue preview, skip/like, "Play on WiiM" toggle, theme adjustment
4. Settings: AI provider config, pool ratios, WiiM IP override

**Phone playback** calls `/stream/{track_id}` directly — same endpoint as WiiM but with the phone as client. The `<audio>` element follows the 302 redirect transparently.

**Switching targets** (phone ↔ WiiM) is a single toggle. Switching to WiiM pauses phone playback, sends current position + queue to backend, which starts the WiiM at the same offset.

---

## 4. State Model (in-memory)

A single `AppState` is shared across handlers via `Arc<AppState>`. All collections are wrapped in appropriate synchronization primitives (`RwLock` for infrequently-written maps, `DashMap` for hot paths).

```rust
pub struct AppState {
    pub config: Config,

    // Auth — re-established at boot from env credentials
    pub qobuz_auth: RwLock<QobuzAuth>,        // app_id, app_secret, user_auth_token

    // Taste profile — rebuilt at boot from Qobuz favorites
    pub taste_profile: RwLock<TasteProfile>,

    // Caches — populated lazily, no persistence
    pub new_releases:    DashMap<ArtistId, CachedReleases>,
    pub similar_artists: DashMap<ArtistId, CachedSimilar>,
    pub track_metadata:  DashMap<TrackId, TrackMetadata>,

    // Live sessions — exist only while a radio is active
    pub sessions: DashMap<SessionId, RadioSession>,

    // Clients
    pub qobuz:   QobuzClient,
    pub lastfm:  LastfmClient,
    pub ai:      Box<dyn MusicAI>,
    pub linkplay: LinkplayClient,
}

pub struct TasteProfile {
    pub artists: HashMap<ArtistId, ArtistWeight>,
    pub last_full_refresh: Instant,
}

pub struct ArtistWeight {
    pub name: String,
    pub base_weight: f32,      // from favorites
    pub session_delta: f32,    // from feedback in current process lifetime
}

pub struct CachedReleases {
    pub releases: Vec<Release>,    // last 6 months, sorted desc
    pub fetched_at: Instant,
}

pub struct CachedSimilar {
    pub artists: Vec<SimilarArtist>,
    pub fetched_at: Instant,
}

pub struct RadioSession {
    pub id: SessionId,
    pub theme_input: String,
    pub theme_tags: Vec<String>,
    pub pool_ratios: PoolRatios,
    pub queue: VecDeque<QueuedTrack>,
    pub history: Vec<PlayedTrack>,
    pub target: Target,            // Phone | Wiim
    pub started_at: Instant,
}

pub struct QueuedTrack {
    pub track_id: TrackId,
    pub pool: Pool,                // Familiar | NewRelease | Discovery
    pub metadata: TrackMetadata,
}

pub struct PlayedTrack {
    pub track_id: TrackId,
    pub pool: Pool,
    pub completed: Option<bool>,   // None until end-of-track event
    pub listened_ms: u64,
}
```

### Cache TTL semantics

TTLs are checked on read:

| Map | TTL | On expiry |
|---|---|---|
| `new_releases` | 24h | Refetch on next access |
| `similar_artists` | 7 days | Refetch on next access |
| `track_metadata` | session lifetime | Never expires while process runs |

A lightweight background task (`tokio::spawn` + `interval`) prunes expired entries every hour to bound memory. Memory footprint stays in the low MB range for a single user.

### Lifecycle

- **Boot**: load config → re-authenticate with Qobuz (email/password from env) → fetch favorites → build `taste_profile`. Caches start empty and fill on demand.
- **Runtime**: all writes go to memory. No disk I/O on the hot path beyond logs.
- **Shutdown**: nothing to persist. Sessions die with the process.
- **Restart**: identical to first boot. Active radios are lost; the user starts a new one.

---

## 5. Key Flows

### 5.1 Initial setup
1. User opens PWA (Qobuz credentials are already in backend env)
2. Backend has authenticated with Qobuz at boot, taste profile is ready
3. User enters a theme and starts a radio

If credentials are missing or auth failed, the PWA surfaces a clear error pointing to backend config.

### 5.2 Starting a radio
```
POST /radio/start
{ theme: "chill folk pour le soir", target: "phone" | "wiim", pool_ratios?: {...} }
```
Backend:
1. Parse theme → tags (dictionary + Last.fm `tag.getSimilar` expansion)
2. Build the three pools:
   - **Familiar**: tracks from `taste_profile` top artists, filtered by tag match
   - **New releases**: `new_releases` cache filtered to artists in profile, scoped to last 6 months
   - **Discovery**: Last.fm `artist.getSimilar` on top profile artists, exclude artists already in `taste_profile`, fetch top tracks, filter by tag
3. Call `MusicAI::rank_candidates` with full candidate set
4. Resolve picked tracks → Qobuz track IDs via `catalog/search`
5. Create session in `AppState.sessions`, populate initial queue
6. If `target=wiim`: generate M3U, send LinkPlay command, return session ID
   If `target=phone`: return first track URL, queue metadata

### 5.3 Sliding window
- Maintain ~20 tracks ahead in the queue
- When fewer than 5 unplayed remain (detected via WiiM polling or phone player events), trigger refresh
- Refresh = rerun the pool builder + ranker with `already_played` set, append to queue
- For WiiM: append to the M3U; rely on LinkPlay's playlist semantics or, if append is unsupported, regenerate-and-replace at end of current track

### 5.4 Implicit feedback
- Track skipped within first 20% → strong negative signal on artist and tags
- Track played to >80% → positive signal
- Updates `taste_profile.artists[artist_id].session_delta` in memory, with a decay function
- Pure in-memory: signals influence the current process lifetime only. On restart, the profile resets to the Qobuz favorites baseline. This is acceptable for the use case.

### 5.5 Stream redirect
```
GET /stream/{track_id}?session={id}
  → backend calls Qobuz track/getFileUrl
  → backend logs the play (for feedback)
  → return 302 Location: <signed Qobuz CDN URL>
```

---

## 6. Configuration

`config.toml` (with environment variable overrides for secrets):

```toml
[server]
host = "0.0.0.0"
port = 8080
public_base_url = "http://192.168.1.10:8080"  # reachable from WiiM

[qobuz]
# email/password set via env: QOBUZ_EMAIL, QOBUZ_PASSWORD
preferred_format_id = 27  # Hi-Res 24-bit

[lastfm]
# api_key via env: LASTFM_API_KEY

[ai]
provider = "workers_ai"  # or "anthropic" | "openai_compat"
model = "@cf/google/gemma-4-26b-a4b-it"
# tokens via env: CLOUDFLARE_ACCOUNT_ID, CLOUDFLARE_API_TOKEN
# or ANTHROPIC_API_KEY / OPENAI_API_KEY + OPENAI_BASE_URL

[radio]
default_pool_ratios = { familiar = 0.60, new_release = 0.25, discovery = 0.15 }
window_size = 20
window_refresh_threshold = 5
new_release_max_age_days = 180

[wiim]
# auto-discovered, or set ip manually:
# ip = "192.168.1.42"
poll_interval_seconds = 5
```

---

## 7. Deployment

- Single Rust binary (`ostinato-radio`), compiled in release mode
- Dockerfile: multi-stage build (cargo-chef for cache), Debian-slim runtime
- Published as a single Docker image, e.g. `ghcr.io/<user>/ostinato-radio:latest`
- No volume mounts required — state is in-memory, config via env vars or a mounted single file
- Reverse-proxied behind existing nginx (TLS termination)
- Backend must remain reachable on local network IP for WiiM, not only via the public reverse-proxied URL — expose the LAN port directly
- Container restart policy: `unless-stopped`. Boot time (incl. Qobuz auth + favorites fetch) is a few seconds.

---

## 8. Security

- Qobuz email/password supplied via env vars at process start, used once per boot to obtain `user_auth_token`
- `user_auth_token` lives in memory only; never written to disk
- Qobuz `app_secret` scraped at runtime, never persisted
- PWA authenticates to backend with a session cookie (HTTP-only, SameSite=Lax, Secure on public URL)
- Backend never returns raw Qobuz tokens to the PWA
- Rate limiting on auth endpoints

---

## 9. Performance & Reliability

- All caches are in-memory with TTL (see §4). No DB round-trips on the hot path.
- AI calls batched: one call per window refresh, not per track
- Stream redirect endpoint must respond in <200ms p99 — never block on Qobuz API; fail fast with 503 and let the player retry
- All third-party API failures degrade gracefully: AI failure → deterministic fallback ranker; Last.fm failure → familiar-only mode; Qobuz failure → surface error
- Memory footprint stays in the low MB range for a single user — verify on first build with a load test (1000+ artists in profile, 10 active sessions)

---

## 10. Open Questions / Empirical Validations

These need to be verified before or during early implementation. They each have a fallback if they don't hold.

1. **Are Qobuz signed stream URLs IP-restricted?**
   - Test: fetch a URL on the backend, `curl` from another device on the LAN.
   - If yes: replace the `302 redirect` with a full streaming proxy. Adds bandwidth cost and complexity but works.

2. **Does LinkPlay reliably handle dynamic M3U with redirects?**
   - Test: serve a 5-track M3U with 302 redirects, play on WiiM, watch behavior.
   - If unreliable: pre-resolve URLs at M3U generation time (accepting expiry risk for long playlists) or switch to UPnP push.

3. **Does LinkPlay support appending to a playing playlist?**
   - If no: regenerate M3U at end of N-1, restart playback at appropriate offset (may cause a brief gap).

4. **Bundle scraping stability?**
   - Qobuz changes the web player periodically. Need a resilient regex set and a fallback to a pinned `app_id/app_secret` from config.

5. **Workers AI free tier limits**
   - Verify daily call budget against expected usage (1 call per radio refresh × multiple sessions per day).

---

## 11. Implementation Order (suggested)

1. **Backend skeleton + Qobuz auth + favorites read** — `AppState` initialization, proves credential pipeline works
2. **Stream redirect endpoint + phone playback PoC** (skip everything else, just `<audio>` pointed at backend)
3. **LinkPlay client + WiiM playback PoC** (hardcoded track list)
4. **Last.fm client + pool builder** (introduces in-memory caches)
5. **AI ranking with one provider (Workers AI)**
6. **Sliding window**
7. **PWA full UX**
8. **Feedback loop and `taste_profile` updates**
9. **Other AI providers**
10. **Polish, cache pruning task, observability**

---

## 12. Out of scope explicitly

- Multi-user, multi-tenant operation
- iOS support beyond PWA install
- Apple Music / Spotify / Tidal — Qobuz is the only backend service
- Audio file downloads, transcoding, format conversion
- Lyrics, radio shows, podcasts
