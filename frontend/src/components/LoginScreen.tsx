import React, { useState } from 'react'
import { useAuth } from '../AuthContext'

interface Props {
  onLogin: () => void
}

export default function LoginScreen({ onLogin }: Props) {
  const { login } = useAuth()
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (!password.trim()) return
    setLoading(true)
    setError(null)
    try {
      await login(password)
      onLogin()
    } catch (e: any) {
      setError(e.message || 'Login failed')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="screen center">
      <h1>Ostinato Radio</h1>
      <p style={{ color: 'var(--muted)', marginBottom: 24 }}>
        Password protected. Enter your password to continue.
      </p>

      <form onSubmit={handleSubmit} style={{ width: '100%', maxWidth: 320 }}>
        <input
          className="theme-input"
          type="password"
          placeholder="Password"
          value={password}
          onChange={(e) => setPassword(e.target.value)}
        />
        {error && (
          <p className="error" style={{ marginTop: 8, fontSize: 14 }}>
            {error}
          </p>
        )}
        <button className="primary-btn" disabled={loading || !password.trim()}>
          {loading ? 'Logging in...' : 'Log in'}
        </button>
      </form>
    </div>
  )
}
