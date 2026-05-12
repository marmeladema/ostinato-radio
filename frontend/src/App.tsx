import { useEffect, useState } from 'react'
import { AuthProvider, useAuth } from './AuthContext'
import { checkAuth, startOauth } from './api'
import type { AuthStatus } from './api'
import Home from './components/Home'
import LoginScreen from './components/LoginScreen'
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

function AppInner() {
  const { token, logout } = useAuth()
  const [view, setView] = useState<View>('home')
  const [radio, setRadio] = useState<RadioData | null>(null)
  const [authState, setAuthState] = useState<AuthStatus | null>(null)
  const [showLogin, setShowLogin] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [waitingForOAuth, setWaitingForOAuth] = useState(false)

  useEffect(() => {
    const poll = () => {
      checkAuth().then((data) => {
        setAuthState(data)
        if (data.has_password && !token) {
          setShowLogin(true)
        }
        if (data.authenticated) {
          setWaitingForOAuth(false)
        }
      })
    }
    poll()
    const interval = setInterval(poll, 3000)
    return () => clearInterval(interval)
  }, [token])

  const handleStartOauth = async () => {
    const url = await startOauth()
    if (url) {
      window.open(url, '_blank')
      setWaitingForOAuth(true)
    } else {
      setError('Failed to start Qobuz authentication')
    }
  }

  if (authState === null) {
    return <div className="loading">Loading...</div>
  }

  if (!authState.authenticated) {
    return (
      <div className="screen center">
        <h1>Ostinato Radio</h1>
        <p>Connect your Qobuz account to get started.</p>
        {waitingForOAuth ? (
          <p className="info">Waiting for authentication... Please complete the sign-in in the new tab.</p>
        ) : (
          <button onClick={handleStartOauth} className="primary-btn">Connect with Qobuz</button>
        )}
        {error && <p className="error">{error}</p>}
      </div>
    )
  }

  if (showLogin) {
    return <LoginScreen onLogin={() => setShowLogin(false)} />
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
        {view === 'settings' && <Settings onLogout={logout} />}
      </main>
    </div>
  )
}

function App() {
  return (
    <AuthProvider>
      <AppInner />
    </AuthProvider>
  )
}

export default App
