const BASE = '' // Proxied by Vite dev server; in production, same origin

export interface AuthStatus {
  authenticated: boolean
  has_password: boolean
  message: string
  display_name?: string
  email?: string
  country_code?: string
  subscription?: string
}

export async function checkAuth(): Promise<AuthStatus> {
  try {
    const res = await fetch(`${BASE}/auth/status`)
    if (!res.ok) return { authenticated: false, has_password: false, message: 'Error checking auth' }
    const data = await res.json()
    return data as AuthStatus
  } catch {
    return { authenticated: false, has_password: false, message: 'Backend unreachable' }
  }
}

export async function startOauth(): Promise<string | null> {
  try {
    const res = await fetch(`${BASE}/auth/start`)
    if (!res.ok) return null
    const data = await res.json()
    return data.oauth_url as string
  } catch {
    return null
  }
}

export interface StartRadioBody {
  theme: string
  target: 'phone' | 'wiim'
  pool_ratios?: {
    familiar: number
    new_release: number
    discovery: number
  }
}

export async function startRadio(
  token: string | null,
  body: StartRadioBody
) {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' }
  if (token) headers['Authorization'] = `Bearer ${token}`

  const res = await fetch(`${BASE}/radio/start`, {
    method: 'POST',
    headers,
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Start radio failed' }))
    throw new Error(err.error)
  }
  return res.json()
}

export async function getSessionStatus(token: string | null, sessionId: string) {
  const headers: Record<string, string> = {}
  if (token) headers['Authorization'] = `Bearer ${token}`

  const res = await fetch(`${BASE}/radio/${sessionId}`, { headers })
  if (!res.ok) throw new Error('Failed to fetch session')
  return res.json()
}

export async function nextTrack(token: string | null, sessionId: string) {
  const headers: Record<string, string> = {}
  if (token) headers['Authorization'] = `Bearer ${token}`

  const res = await fetch(`${BASE}/radio/${sessionId}/next`, {
    method: 'POST',
    headers,
  })
  if (!res.ok) throw new Error('Failed to advance queue')
  return res.json()
}

export async function submitFeedback(
  token: string | null,
  sessionId: string,
  payload: {
    track_id: string
    action: 'skip' | 'complete' | 'progress'
    progress_ms?: number
    duration_ms?: number
  }
) {
  const headers: Record<string, string> = { 'Content-Type': 'application/json' }
  if (token) headers['Authorization'] = `Bearer ${token}`

  const res = await fetch(`${BASE}/feedback/${sessionId}`, {
    method: 'POST',
    headers,
    body: JSON.stringify(payload),
  })
  if (!res.ok) throw new Error('Feedback failed')
  return res.json()
}

export function streamUrl(trackId: string, sessionId?: string) {
  const q = sessionId ? `?session=${sessionId}` : ''
  return `${BASE}/stream/${trackId}${q}`
}
