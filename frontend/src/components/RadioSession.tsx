import React, { useEffect, useRef, useState } from 'react'
import { streamUrl, nextTrack, submitFeedback, getSessionStatus } from '../api'
import { useAudio } from '../hooks/useAudio'
import type { RadioData } from '../App'

interface Props {
  radio: RadioData
  onBack: () => void
  onError: (msg: string) => void
}

export default function RadioSession({ radio, onBack, onError }: Props) {
  const { playing, play, pause, resume, currentTime, duration } = useAudio()
  const [queue, setQueue] = useState(radio.queue)
  const [current, setCurrent] = useState(radio.queue[0] || null)
  const [target, setTarget] = useState(radio.target)
  const startTimeRef = useRef<number>(Date.now())

  useEffect(() => {
    if (current && target === 'phone') {
      play(streamUrl(current.track_id, radio.session_id))
    }
  }, [current, target])

  const handleSkip = async () => {
    if (!current) return
    await submitFeedback(radio.session_id, {
      track_id: current.track_id,
      action: 'skip',
      progress_ms: Math.floor(currentTime * 1000),
      duration_ms: Math.floor((duration || currentTime) * 1000),
    }).catch(() => {})
    await advance()
  }

  const handleComplete = async () => {
    if (!current) return
    await submitFeedback(radio.session_id, {
      track_id: current.track_id,
      action: 'complete',
      duration_ms: Math.floor((duration || currentTime) * 1000),
    }).catch(() => {})
    await advance()
  }

  const advance = async () => {
    try {
      const n = await nextTrack(radio.session_id)
      setCurrent(n)
      // Refresh full session to get updated queue
      const status = await getSessionStatus(radio.session_id)
      if (status.current_track) {
        setCurrent(status.current_track)
      }
      // Rebuild queue from status if possible; otherwise drop first
      setQueue((q) => q.slice(1))
    } catch (e: any) {
      onError(e.message || 'Queue ended')
      setCurrent(null)
    }
  }

  const toggleTarget = () => {
    const next = target === 'phone' ? 'wiim' : 'phone'
    setTarget(next)
    if (next === 'phone') {
      if (current) play(streamUrl(current.track_id, radio.session_id))
    } else {
      pause()
    }
  }

  const cover = current?.image_url || ''

  return (
    <div className="screen">
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <button onClick={onBack} style={{ background: 'none', border: 'none', color: 'var(--text)' }}>{'← Back'}</button>
        <span style={{ color: 'var(--muted)', fontSize: 12 }}>Session: {radio.session_id.slice(0, 8)}</span>
      </div>

      <div className="player-card">
        {cover ? <img className="cover" src={cover} alt="cover" /> : <div className="cover" />}
        <div style={{ marginTop: 12 }}>
          <div className="title" style={{ fontSize: 18, fontWeight: 700 }}>{current?.title || '—'}</div>
          <div style={{ color: 'var(--muted)' }}>{current?.artist || '—'}</div>
          <div style={{ color: 'var(--muted)', fontSize: 12 }}>{current?.album || '—'} · {current?.pool}</div>
        </div>

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
        <h3>Up next ({queue.length - 1})</h3>
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
    </div>
  )
}
