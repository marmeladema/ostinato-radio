import { useState } from 'react'

interface Props {
  onLogout: () => void
}

export default function Settings({ onLogout }: Props) {
  const [wiimIp, setWiimIp] = useState(localStorage.getItem('wiimIp') || '')
  const [saved, setSaved] = useState(false)

  const handleSave = () => {
    localStorage.setItem('wiimIp', wiimIp)
    setSaved(true)
    setTimeout(() => setSaved(false), 2000)
  }

  return (
    <div className="screen animate-in">
      <h2>Settings</h2>

      <div className="settings-group">
        <label>WiiM IP Override</label>
        <input
          placeholder="192.168.1.42"
          value={wiimIp}
          onChange={(e) => setWiimIp(e.target.value)}
        />
        <p style={{ fontSize: 13, marginTop: 8 }}>
          Leave empty for auto-discovery.
        </p>
      </div>

      <div className="settings-group">
        <label>Pool Ratios (default)</label>
        <p style={{ fontSize: 13 }}>
          Familiar 60% · New Releases 25% · Discovery 15%
        </p>
        <p style={{ fontSize: 13, marginTop: 4 }}>
          Adjust via backend <code>config.toml</code> temporarily.
        </p>
      </div>

      <div className="settings-group">
        <label>AI Provider</label>
        <p style={{ fontSize: 13 }}>
          Configured on the backend via <code>config.toml</code> / env vars.
        </p>
      </div>

      <button className="primary-btn" onClick={handleSave}>
        {saved ? 'Saved!' : 'Save Settings'}
      </button>

      <button className="secondary-btn" onClick={onLogout}>
        Log out
      </button>
    </div>
  )
}
