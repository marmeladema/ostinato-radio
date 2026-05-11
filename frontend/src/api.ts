const BASE = '' // Proxied by Vite dev server; in production, same origin

export async function checkAuth(): Promise<{ authenticated: boolean; has_password: boolean }> {
  try {
    const res = await fetch(`${BASE}/auth/status`)
    if (!res.ok) return { authenticated: false, has_password: false }
    const data = await res.json()
    return { authenticated: data.authenticated, has_password: data.has_password }
  } catch {
    return { authenticated: false, has_password: false }
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
