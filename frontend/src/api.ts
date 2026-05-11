const BASE = '' // Proxied by Vite dev server; in production, same origin

export async function checkAuth(): Promise<boolean> {
  try {
    const res = await fetch(`${BASE}/auth/status`)
    if (!res.ok) return false
    const data = await res.json()
    return data.authenticated
  } catch {
    return false
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

export async function startRadio(body: StartRadioBody) {
  const res = await fetch(`${BASE}/radio/start`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
  if (!res.ok) {
    const err = await res.json().catch(() => ({ error: 'Start radio failed' }))
    throw new Error(err.error)
  }
  return res.json()
}

export async function getSessionStatus(sessionId: string) {
  const res = await fetch(`${BASE}/radio/${sessionId}`)
  if (!res.ok) throw new Error('Failed to fetch session')
  return res.json()
}

export async function nextTrack(sessionId: string) {
  const res = await fetch(`${BASE}/radio/${sessionId}/next`, { method: 'POST' })
  if (!res.ok) throw new Error('Failed to advance queue')
  return res.json()
}

export async function submitFeedback(
  sessionId: string,
  payload: {
    track_id: string
    action: 'skip' | 'complete' | 'progress'
    progress_ms?: number
    duration_ms?: number
  }
) {
  const res = await fetch(`${BASE}/feedback/${sessionId}`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(payload),
  })
  if (!res.ok) throw new Error('Feedback failed')
  return res.json()
}

export function streamUrl(trackId: string, sessionId?: string) {
  const q = sessionId ? `?session=${sessionId}` : ''
  return `${BASE}/stream/${trackId}${q}`
}
