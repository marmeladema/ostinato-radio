import React, { useEffect, useState } from 'react'
import { API_BASE, checkAuth } from './api'
import Home from './components/Home'
import RadioSession from './components/RadioSession'
import Settings from './components/Settings'

export type View = 'home' | 'radio' | 'settings'

export interface RadioData {
  session_id: string
  theme_tags: string[]
  queue: {
    track_id: string
    title: string
    artist: string
    album: string
    image_url?: string
    pool: string
  }[]
  target: string
}

function App() {
  const [view, setView] = useState<View>('home')
  const [radio, setRadio] = useState<RadioData | null>(null)
  const [authOk, setAuthOk] = useState<boolean | null>(null)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    checkAuth().then((ok) => setAuthOk(ok)).catch(() => setAuthOk(false))
  }, [])

  if (authOk === null) {
    return <div className="loading">Loading...</div>
  }

  if (!authOk) {
    return (
      <div className="screen center">
        <h1>Ostinato Radio</h1>
        <p className="error">Backend not authenticated. Please configure Qobuz credentials and restart the server.</p>
      </div>
    )
  }

  return (
    <div className="app">
      {error && <div className="toast error" onClick={() => setError(null)}>{error}</div>}
      <nav className="bottom-nav">
        <button className={view === 'home' ? 'active' : ''} onClick={() => setView('home')}>Home</button>
        <button className={view === 'radio' && radio ? 'active' : ''} onClick={() => radio && setView('radio')}>Radio</button>
        <button className={view === 'settings' ? 'active' : ''} onClick={() => setView('settings')}>Settings</button>
      </nav>

      <main>
        {view === 'home' && (
          <Home
            onRadioStarted={(data) => {
              setRadio(data)
              setView('radio')
            }}
            onError={setError}
          />
        )}
        {view === 'radio' && radio && (
          <RadioSession
            radio={radio}
            onBack={() => setView('home')}
            onError={setError}
          />
        )}
        {view === 'settings' && <Settings onError={setError} />}
      </main>
    </div>
  )
}

export default App
