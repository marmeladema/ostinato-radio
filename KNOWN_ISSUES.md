# Known Gaps, Stubs & Empirical Validations

> Document tracking what's implemented, what's stubbed, and what needs real-world testing.

---

## Implemented ✅

- [x] Rust project skeleton with Axum server
- [x] Config loading (TOML + env vars)
- [x] Unified error handling (`AppError` + `IntoResponse`)
- [x] In-memory AppState with taste profile, caches, sessions
- [x] Qobuz bundle scraper (`bundle.js` → app_id / app_secret)
- [x] Qobuz login (email/password → user_auth_token)
- [x] Qobuz favorites fetch → taste profile build at boot
- [x] Qobuz search, track metadata, artist with albums, stream URL (getFileUrl)
- [x] Qobuz API request signing (MD5-based per Qobuz spec)
- [x] Last.fm client (`artist.getSimilar` — used in discovery pool)
- [x] Pool builder (Familiar + New Releases + Discovery)
- [x] AI provider abstraction (`MusicAI` trait)
- [x] Deterministic fallback ranker (pool-ordered)
- [x] **Workers AI** provider with prompt construction and JSON response parsing
- [x] Anthropic provider skeleton (wired, returns "not yet implemented")
- [x] OpenAI-compatible provider skeleton (wired, returns "not yet implemented")
- [x] Radio session creation (`POST /radio/start`)
- [x] Session status (`GET /radio/{session_id}`)
- [x] Queue advance (`POST /radio/{session_id}/next`)
- [x] Stream redirect (`GET /stream/{track_id}?session={id}`) with 302 to Qobuz CDN
- [x] M3U playlist generator for WiiM (`GET /playback/wiim/{session_id}.m3u`)
- [x] LinkPlay / WiiM control commands (play, pause, resume, stop, next, prev, volume)
- [x] Feedback submission (`POST /feedback/{session_id}`) — skip/complete/progress
- [x] Cache pruning background task (hourly, for `new_releases` and `similar_artists`)
- [x] Frontend PWA (React 18 + Vite + TypeScript)
  - [x] Home screen with theme input + preset buttons
  - [x] Phone vs WiiM target selection
  - [x] Radio session player with current track display
  - [x] Skip / Complete controls
  - [x] Queue preview
  - [x] Settings screen
  - [x] Service worker for PWA installability
- [x] Dockerfile with multi-stage build (backend + frontend)
- [x] Static file serving from `frontend/dist` as fallback
- [x] `cargo check`, `cargo clippy`, `cargo fmt` — all clean (zero warnings)

---

## Stubbed / Partially Implemented ⚠️

### AI Providers
- **Anthropic** (`src/providers/ai/anthropic.rs`) — struct and trait impl exist, but `rank_candidates` returns `Err("Anthropic not yet implemented")`. Falls back to deterministic ranker.
- **OpenAI-compatible** (`src/providers/ai/openai_compat.rs`) — same pattern as Anthropic.

### Pool Builder
- **New Releases pool** — currently searches artists and creates candidates from album titles, but does **not** fetch the actual album track list. Search results may return albums instead of individual tracks.
- **Discovery pool** — depends on Qobuz search for candidate resolution; if an artist name doesn't yield good Qobuz matches, the pool may be sparse.

### Feedback Loop
- **Taste profile adjustment** — feedback is recorded in session history (`PlayedTrack.completed`, `.listened_ms`), but `session_delta` on `ArtistWeight` is never actually updated. The profile resets on every server restart.

### Sliding Window
- **Auto-refresh not wired** — `window_refresh_threshold` is configured but never checked. The queue is populated once at session start (~20 tracks) and only shrinks as the user advances. There is no automatic rerun of `build_pools` + `ranker` when queue drops below threshold.
- **`append_to_session`** exists but is never called — it was written for the sliding window refresh that hasn't been hooked up yet.

### Last.fm
- **`get_top_tracks`**, **`get_top_tracks_by_tag`**, **`get_similar_tags`** — implemented and tested for compilation, but never called by the pool builder. The discovery pool currently only uses `artist.getSimilar` followed by Qobuz search.

### LinkPlay / WiiM
- **No polling loop** — `poll_interval_secs` is configured but there's no background task polling `getPlayerStatus`. This means:
  - No automatic sliding window trigger based on WiiM playback position.
  - No implicit feedback generated from WiiM playback events.
- **M3U append vs regenerate** — currently generates a static M3U at session start. If LinkPlay doesn't support playlist append, the WiiM will stop when the initial M3U ends.
- **WiiM auto-discovery** — not implemented. IP must be set in `config.toml`/`OSTINATO__WIIM__IP`.
- **`set_ip`**, **`get_player_status`**, **`PlayerStatus`** — implemented but unused.

---

## Open Questions / Empirical Validations 🔬

These are from spec §10 and need real-world testing before fully relying on the behavior:

### 1. Are Qobuz signed stream URLs IP-restricted?
- **Test**: Fetch a URL on the backend, then `curl` it from another device on the LAN.
- **Current behavior**: Backend returns a 302 redirect to the signed Qobuz CDN URL. Player follows it.
- **Fallback if yes**: Replace 302 with a full streaming proxy (adds bandwidth cost).
- **Status**: Not empirically verified.

### 2. Does LinkPlay reliably handle dynamic M3U with 302 redirects?
- **Test**: Serve a 5-track M3U with 302 redirects, play on WiiM, observe.
- **Fallback if no**: Pre-resolve URLs at M3U generation time (accepting expiry risk) or switch to UPnP push.
- **Status**: Not empirically verified.

### 3. Does LinkPlay support appending to a playing playlist?
- **Test**: Start a playlist, then try to push/append more items.
- **Fallback if no**: Regenerate M3U + restart playback at same offset (may cause brief gap).
- **Status**: Not empirically verified. Current code does not attempt append.

### 4. Bundle scraping stability?
- **Test**: Monitor if Qobuz changes `play.qobuz.com` web player structure.
- **Fallback**: Pinned `app_id`/`app_secret` from env vars (`QOBUZ_APP_ID` / `QOBUZ_APP_SECRET`).
- **Status**: Working today, but regex set is minimal. Could break.

### 5. Workers AI free tier limits?
- **Test**: Track daily call budget against usage (1 call per window refresh × sessions per day).
- **Status**: Untested. Currently using deterministic fallback if credentials missing.

---

## Minor Technical Debt

- **Config env var prefix** is `OSTINATO__` (double underscore), but many natural env vars like `QOBUZ_EMAIL` are read separately in code, not through the config crate. This is intentional but slightly inconsistent.
- **No rate limiting** on any endpoint yet.
- **CORS is fully permissive** (`CorsLayer::permissive()`) — fine for local dev, tighten for production.
- **Frontend proxy config** in `vite.config.ts` proxies individual route prefixes. Could be consolidated with `/api` prefix on backend.
- **Frontend `BASE` URL in `api.ts`** is empty string — relies on Vite proxy in dev, same-origin in production.
- **No tests** — no unit or integration tests written yet.

---

## Recommended Next Steps (in order)

1. **Empirical validation #2** — test dynamic M3U with 302 redirects on actual WiiM hardware.
2. **Implement Anthropic AI provider** ( Messages API, structured JSON output).
3. **Wire sliding window auto-refresh** — background task or audio `ended` event?
   - For phone: HTML5 audio `ended` event → trigger pool rebuild if queue < threshold.
   - For WiiM: polling loop + position tracking.
4. **Fetch album tracks for New Releases pool** — currently only gets album metadata, not individual tracks.
5. **Write tests** for Qobuz signing, pool builder, ranker fallback.
