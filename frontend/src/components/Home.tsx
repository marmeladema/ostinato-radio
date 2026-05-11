import React, { useState } from 'react'
import { useAuth } from '../AuthContext'
import { startRadio, StartRadioBody } from '../api'
import type { RadioData } from '../App'

const PRESETS = [
  'chill folk soir',
  'running',
  'winter ambient',
  'shoegaze',
  'jazz evening',
  'electronic upbeat',
]

interface Props {
  onRadioStarted: (data: RadioData) => void
  onError: (msg: string) => void
}

export default function Home({ onRadioStarted, onError }: Props) {
  const { token } = useAuth()
  const [theme, setTheme] = useState('')
  const [loading, setLoading] = useState(false)
  const [target, setTarget] = useState<'phone' | 'wiim'>('phone')

  const handleStart = async () => {
    if (!theme.trim()) return
    setLoading(true)
    try {
      const body: StartRadioBody = { theme: theme.trim(), target }
      const data = await startRadio(token, body)
      onRadioStarted(data as RadioData)
    } catch (e: any) {
      onError(e.message || 'Failed to start radio')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="screen">
      <h1>Ostinato Radio</h1>
      <p style={{ color: 'var(--muted)' }}>Personalized infinite radio over Qobuz</p>

      <div className="presets">
        {PRESETS.map((p) => (
          <button key={p} className="preset-btn" onClick={() => setTheme(p)}>
            {p}
          </button>
        ))}
      </div>

      <input
        className="theme-input"
        placeholder="Enter a theme: mood, genre, occasion..."
        value={theme}
        onChange={(e) => setTheme(e.target.value)}
        onKeyDown={(e) => e.key === 'Enter' && handleStart()}
      />

      <div style={{ marginTop: 12, display: 'flex', gap: 8 }}>
        <label style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 8, padding: 10, background: 'var(--surface)', borderRadius: 'var(--radius)', cursor: 'pointer' }}>
          <input type="radio" name="target" checked={target === 'phone'} onChange={() => setTarget('phone')} />
          Play on phone
        </label>
        <label style={{ flex: 1, display: 'flex', alignItems: 'center', gap: 8, padding: 10, background: 'var(--surface)', borderRadius: 'var(--radius)', cursor: 'pointer' }}>
          <input type="radio" name="target" checked={target === 'wiim'} onChange={() => setTarget('wiim')} />
          Play on WiiM
        </label>
      </div>

      <button className="primary-btn" disabled={loading || !theme.trim()} onClick={handleStart}>
        {loading ? 'Starting...' : 'Start Radio'}
      </button>
    </div>
  )
}
