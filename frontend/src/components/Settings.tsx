import React, { useState } from 'react'

interface Props {
  onError: (msg: string) => void
}

export default function Settings({ onError }: Props) {
  const [wiimIp, setWiimIp] = useState(localStorage.getItem('wiimIp') || '')
  const [saved, setSaved] = useState(false)

  const handleSave = () => {
    localStorage.setItem('wiimIp', wiimIp)
    setSaved(true)
    setTimeout(() => setSaved(false), 2000)
  }

  return (
    <div className="screen">
      <h2>Settings</h2>

      <div className="settings-group">
        <label>WiiM IP Override</label>
        <input
          placeholder="192.168.1.42"
          value={wiimIp}
          onChange={(e) => setWiimIp(e.target.value)}
        />
        <p style={{ color: 'var(--muted)', fontSize: 12, marginTop: 6 }}>
          Leave empty for auto-discovery.
        </p>
      </div>

      <div className="settings-group">
        <label>Pool Ratios (default)</label>
        <p style={{ color: 'var(--muted)', fontSize: 12 }}>
          Familiar 60% · New Releases 25% · Discovery 15%
        </p>
        <p style={{ color: 'var(--muted)', fontSize: 12 }}>
          Adjust via backend config.toml temporarily.
        </p>
      </div>

      <div className="settings-group">
        <label>AI Provider</label>
        <p style={{ color: 'var(--muted)', fontSize: 12 }}>
          Configured on the backend via config.toml / env vars.
        </p>
      </div>

      <button className="primary-btn" onClick={handleSave}>
        {saved ? 'Saved!' : 'Save'}
      </button>
    </div>
  )
}
