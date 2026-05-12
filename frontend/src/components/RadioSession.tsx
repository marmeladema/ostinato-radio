import { useEffect, useState, useCallback } from 'react'
import { useAuth } from '../AuthContext'
import { nextTrack, submitFeedback, getSessionStatus, getTrackStreamInfo } from '../api'
import { useAudio } from '../hooks/useAudio'
import type { RadioData } from '../App'
import type { StreamInfo } from '../api'

interface Props {
  radio: RadioData
  onBack: () => void
  onError: (msg: string) => void
}

function fmtTime(s: number) {
  const m = Math.floor(s / 60)
  const sec = Math.floor(s % 60)
  return `${m}:${sec.toString().padStart(2, '0')}`
}

function fmtQuality(info: StreamInfo | null) {
  if (!info) return ''
  const parts = [info.format]
  if (info.sampling_rate) parts.push(`${info.sampling_rate}kHz`)
  if (info.bit_depth) parts.push(`${info.bit_depth}-bit`)
  return parts.join(' · ')
}

function readyStateName(n: number) {
  switch (n) {
    case 0: return 'HAVE_NOTHING'
    case 1: return 'HAVE_METADATA'
    case 2: return 'HAVE_CURRENT_DATA'
    case 3: return 'HAVE_FUTURE_DATA'
    case 4: return 'HAVE_ENOUGH_DATA'
    default: return `UNKNOWN(${n})`
  }
}

function networkStateName(n: number) {
  switch (n) {
    case 0: return 'NETWORK_EMPTY'
    case 1: return 'NETWORK_IDLE'
    case 2: return 'NETWORK_LOADING'
    case 3: return 'NETWORK_NO_SOURCE'
    default: return `UNKNOWN(${n})`
  }
}

function getCodecSupport() {
  const a = document.createElement('audio')
  const tests = [
    'audio/flac',
    'audio/flac; codecs="flac"',
    'audio/mpeg',
    'audio/mpeg; codecs="mp3"',
    'audio/mp4; codecs="flac"',
    'audio/wav',
    'audio/ogg; codecs="flac"',
  ]
  return tests.map((type) => ({
    type,
    support: a.canPlayType(type) || 'no',
  }))
}

export default function RadioSession({ radio, onBack, onError }: Props) {
  const { token } = useAuth()
  const { playing, play, pause, resume, currentTime, duration, error, loading, diagnostics, setOnEnded } = useAudio()
  const [queue, setQueue] = useState(radio.queue)
  const [current, setCurrent] = useState<typeof radio.queue[0] | null>(radio.queue[0] || null)
  const [target, setTarget] = useState(radio.target)
  const [streamInfo, setStreamInfo] = useState<StreamInfo | null>(null)
  const [showDebug, setShowDebug] = useState(false)
  const [fetchTest, setFetchTest] = useState<any | null>(null)

  const advance = useCallback(async () => {
    try {
      const n = await nextTrack(token, radio.session_id)
      setCurrent(n)
      setQueue((q) => q.slice(1))
      setFetchTest(null)
    } catch (e: any) {
      onError(e.message || 'Queue ended')
      setCurrent(null)
    }
  }, [token, radio.session_id, onError])

  const handleComplete = useCallback(async () => {
    if (!current) return
    // Fire-and-forget feedback so the UI advances immediately
    submitFeedback(token, radio.session_id, {
      track_id: current.track_id,
      action: 'complete',
      duration_ms: Math.floor((duration || currentTime) * 1000),
    }).catch(() => {})
    await advance()
  }, [current, token, radio.session_id, duration, currentTime, advance])

  const handleSkip = useCallback(async () => {
    if (!current) return
    // Fire-and-forget feedback so the UI advances immediately
    submitFeedback(token, radio.session_id, {
      track_id: current.track_id,
      action: 'skip',
      progress_ms: Math.floor(currentTime * 1000),
      duration_ms: Math.floor((duration || currentTime) * 1000),
    }).catch(() => {})
    await advance()
  }, [current, token, radio.session_id, currentTime, duration, advance])

  useEffect(() => {
    setOnEnded(handleComplete)
    return () => setOnEnded(null)
  }, [setOnEnded, handleComplete])

  const startPhonePlayback = useCallback(async (trackId: string) => {
    try {
      const info = await getTrackStreamInfo(token, trackId, 5) // MP3 320
      setStreamInfo(info)
      // Play the raw Qobuz CDN URL directly to avoid 302 redirect issues
      // on Chrome Android's <audio> element.
      play(info.url)
      return info
    } catch (e: any) {
      onError(`Stream error: ${e?.message || 'unknown'}`)
      setStreamInfo(null)
      throw e
    }
  }, [token, play, onError])

  useEffect(() => {
    if (current && target === 'phone') {
      startPhonePlayback(current.track_id)
    }
  }, [current, target, startPhonePlayback])

  const toggleTarget = useCallback(() => {
    setTarget((prev) => {
      const next = prev === 'phone' ? 'wiim' : 'phone'
      if (next === 'phone') {
        if (current) startPhonePlayback(current.track_id)
      } else {
        pause()
      }
      return next
    })
  }, [current, startPhonePlayback, pause])

  const runFetchTest = useCallback(async () => {
    if (!current) return
    const start = performance.now()
    try {
      const info = await getTrackStreamInfo(token, current.track_id, 5)
      setFetchTest({
        ...info,
        elapsed: Math.round(performance.now() - start),
      })
    } catch (e: any) {
      setFetchTest({ error: e?.message || 'fetch failed', elapsed: Math.round(performance.now() - start) })
    }
  }, [current, token])

  const cover = current?.image_url || ''

  if (!current) {
    return (
      <div className="screen center">
        <h2>End of queue</h2>
        <p style={{ color: 'var(--muted)', marginTop: 8 }}>All tracks in this session have been played.</p>
        <button className="primary-btn" style={{ marginTop: 16 }} onClick={onBack}>
          Start New Radio
        </button>
      </div>
    )
  }

  return (
    <div className="screen">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <button onClick={onBack} style={{ background: 'none', border: 'none', color: 'var(--text)' }}>{'← Back'}</button>
        <span style={{ color: 'var(--muted)', fontSize: 12 }}>Session: {radio.session_id.slice(0, 8)}</span>
      </div>

      <div className="player-card">
        {cover ? <img className="cover" src={cover} alt="cover" /> : <div className="cover" />}
        <div style={{ marginTop: 12 }}>
          <div className="title" style={{ fontSize: 18, fontWeight: 700 }}>{current.title}</div>
          <div style={{ color: 'var(--muted)' }}>{current.artist}</div>
          <div style={{ color: 'var(--muted)', fontSize: 12 }}>{current.album} · {current.pool}</div>
          {streamInfo && (
            <div style={{ color: 'var(--accent)', fontSize: 11, marginTop: 4, fontVariantNumeric: 'tabular-nums' }}>
              {fmtQuality(streamInfo)}
            </div>
          )}
        </div>

        {target === 'phone' && (
          <div className="progress-wrap">
            <div className="progress-bar">
              <div
                className="progress-fill"
                style={{ width: `${duration > 0 ? (currentTime / duration) * 100 : 0}%` }}
              />
            </div>
            <div className="progress-time">
              <span>{fmtTime(currentTime)}</span>
              <span>{fmtTime(duration || (current.duration ?? 0))}</span>
            </div>
            {loading && <div style={{ fontSize: 11, color: 'var(--muted)', textAlign: 'center' }}>Loading…</div>}
            {error && <div style={{ fontSize: 11, color: 'var(--accent)', textAlign: 'center' }}>{error}</div>}
          </div>
        )}

        <div className="controls">
          <button className="ctrl-btn" onClick={handleSkip}>⏭</button>
          {target === 'phone' && (
            <button className="ctrl-btn large" onClick={() => (playing ? pause() : resume())}>
              {playing ? '⏸' : '▶'}
            </button>
          )}
          <button className="ctrl-btn" onClick={handleComplete}>✓</button>
        </div>

        <button className="primary-btn" onClick={toggleTarget}>
          {target === 'phone' ? 'Switch to WiiM' : 'Switch to Phone'}
        </button>
      </div>

      <div className="queue">
        <h3>Up next ({Math.max(0, queue.length - 1)})</h3>
        {queue.slice(1, 6).map((t) => (
          <div key={t.track_id} className="queue-item">
            {t.image_url ? <img src={t.image_url} alt="" /> : <div style={{ width: 48, height: 48, borderRadius: 8, background: 'var(--surface-2)' }} />}
            <div className="meta">
              <div className="title">{t.title}</div>
              <div className="artist">{t.artist} · {t.pool}</div>
            </div>
          </div>
        ))}
      </div>

      <div style={{ marginTop: 16 }}>
        <button
          className="primary-btn"
          style={{ fontSize: 12, padding: '6px 12px' }}
          onClick={() => setShowDebug((s) => !s)}
        >
          {showDebug ? 'Hide Debug Info' : 'Show Debug Info'}
        </button>
      </div>

      {showDebug && (
        <div style={{ marginTop: 12, fontSize: 11, fontFamily: 'monospace', background: 'var(--surface-2)', padding: 12, borderRadius: 8, overflow: 'auto' }}>
          <div style={{ fontWeight: 'bold', marginBottom: 8 }}>Audio Element State</div>
          <div>readyState: {diagnostics.readyState} ({readyStateName(diagnostics.readyState)})</div>
          <div>networkState: {diagnostics.networkState} ({networkStateName(diagnostics.networkState)})</div>
          <div>currentSrc: {diagnostics.currentSrc || '(empty)'}</div>
          <div>lastError: {diagnostics.errorCode !== null ? `${diagnostics.errorCode} (${diagnostics.errorMessage})` : 'none'}</div>

          <div style={{ fontWeight: 'bold', marginTop: 12, marginBottom: 8 }}>Browser Codec Support</div>
          {getCodecSupport().map((c) => (
            <div key={c.type}>{c.type}: {c.support}</div>
          ))}

          <div style={{ fontWeight: 'bold', marginTop: 12, marginBottom: 8 }}>Event Log (last 20)</div>
          {diagnostics.events.slice(-20).map((e, i) => (
            <div key={i}>{new Date(e.time).toLocaleTimeString()}.{String(e.time % 1000).padStart(3, '0')} — {e.event}{e.detail ? `: ${e.detail}` : ''}</div>
          ))}

          <div style={{ fontWeight: 'bold', marginTop: 12, marginBottom: 8 }}>Network Fetch Test</div>
          <button className="primary-btn" style={{ fontSize: 11, padding: '4px 8px' }} onClick={runFetchTest}>Fetch stream info from backend</button>
          {fetchTest && (
            <div style={{ marginTop: 4 }}>
              {fetchTest.error ? (
                <div style={{ color: 'var(--accent)' }}>Error: {fetchTest.error} ({fetchTest.elapsed}ms)</div>
              ) : (
                <>
                  <div>format: {fetchTest.format} ({fetchTest.format_id})</div>
                  <div>Qobuz CDN: {fetchTest.url ? 'resolved' : 'missing'}</div>
                  <div>backend elapsed: {fetchTest.elapsed}ms</div>
                  <div style={{ wordBreak: 'break-all', marginTop: 4 }}>direct URL: {fetchTest.url}</div>
                </>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
