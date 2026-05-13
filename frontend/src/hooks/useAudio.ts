import { useRef, useState, useCallback } from 'react'

export interface AudioDiagnostics {
  events: { event: string; time: number; detail?: string }[]
  readyState: number
  networkState: number
  currentSrc: string
  errorCode: number | null
  errorMessage: string | null
}

export function useAudio() {
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const onEndedRef = useRef<(() => void) | null>(null)
  const [playing, setPlaying] = useState(false)
  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [diagnostics, setDiagnostics] = useState<AudioDiagnostics>({
    events: [],
    readyState: 0,
    networkState: 0,
    currentSrc: '',
    errorCode: null,
    errorMessage: null,
  })

  const logEvent = useCallback((event: string, detail?: string) => {
    const entry = { event, time: Date.now(), detail }
    console.log(`[Audio] ${event}${detail ? ': ' + detail : ''}`)
    setDiagnostics((prev) => ({
      ...prev,
      events: [...prev.events.slice(-49), entry], // keep last 50
    }))
  }, [])

  const updateState = useCallback(() => {
    const a = audioRef.current
    if (!a) return
    setDiagnostics((prev) => ({
      ...prev,
      readyState: a.readyState,
      networkState: a.networkState,
      currentSrc: a.currentSrc,
    }))
  }, [])

  const setOnEnded = useCallback((cb: (() => void) | null) => {
    onEndedRef.current = cb
  }, [])

  const play = useCallback((url: string) => {
    if (!url) {
      logEvent('play() rejected', 'url is empty — cannot play')
      setError('Cannot play: empty stream URL')
      setLoading(false)
      return
    }

    // Pause and orphan the previous audio element. We intentionally do NOT
    // clear its src or call load() — that would fire spurious error events
    // on the old element which share the same React state setters.
    const previous = audioRef.current
    if (previous) {
      previous.pause()
      logEvent('orphaned previous audio element')
    }

    setError(null)
    setLoading(true)

    const a = new Audio()

    a.addEventListener('loadstart', () => { logEvent('loadstart'); updateState() })
    a.addEventListener('loadedmetadata', () => {
      logEvent('loadedmetadata', `duration=${a.duration}`)
      setDuration(a.duration)
      setLoading(false)
      updateState()
    })
    a.addEventListener('loadeddata', () => { logEvent('loadeddata'); updateState() })
    a.addEventListener('canplay', () => { logEvent('canplay'); updateState() })
    a.addEventListener('canplaythrough', () => { logEvent('canplaythrough'); updateState() })
    a.addEventListener('play', () => {
      logEvent('play')
      setPlaying(true)
      setError(null)
      updateState()
    })
    a.addEventListener('playing', () => { logEvent('playing'); setLoading(false); updateState() })
    a.addEventListener('pause', () => { logEvent('pause'); setPlaying(false); updateState() })
    a.addEventListener('waiting', () => { logEvent('waiting'); setLoading(true); updateState() })
    a.addEventListener('stalled', () => { logEvent('stalled'); updateState() })
    a.addEventListener('suspend', () => { logEvent('suspend'); updateState() })
    a.addEventListener('abort', () => { logEvent('abort'); updateState() })
    a.addEventListener('timeupdate', () => setCurrentTime(a.currentTime))
    a.addEventListener('ended', () => {
      logEvent('ended')
      setPlaying(false)
      onEndedRef.current?.()
    })
    a.addEventListener('error', () => {
      const err = a.error
      const code = err?.code ?? 0
      const msg = err?.message ?? 'unknown'
      const detail = `code=${code}(${mediaErrorName(code)}) message=${msg}`
      logEvent('error', detail)

      // Only apply the error if this element is still the current one.
      // If we have already moved to a new track, the old element's error
      // should not overwrite the new track's state.
      if (audioRef.current === a) {
        setError(`Audio error ${code}: ${mediaErrorName(code)} – ${msg}`)
        setLoading(false)
        setPlaying(false)
        setDiagnostics((prev) => ({
          ...prev,
          errorCode: code,
          errorMessage: msg,
        }))
        updateState()
      } else {
        logEvent('error ignored', 'element was already replaced')
      }
    })

    audioRef.current = a

    logEvent('play()', `url=${url.substring(0, 80)}${url.length > 80 ? '...' : ''}`)

    a.src = url
    a.load()
    logEvent('src assigned & load() called')

    a.play().catch((e: DOMException) => {
      const mediaErr = a.error
      if (mediaErr && audioRef.current === a) {
        const code = mediaErr.code
        const msg = mediaErr.message
        logEvent('play() rejected', `MediaError ${code}(${mediaErrorName(code)}): ${msg}`)
        setLoading(false)
        setError(`Audio error ${code}: ${mediaErrorName(code)} – ${msg}`)
      } else if (audioRef.current === a) {
        const reason = e?.name ?? 'unknown'
        const msg = e?.message ?? ''
        logEvent('play() rejected', `${reason}: ${msg}`)
        setLoading(false)
        setError(`Playback blocked: ${reason} – ${msg}`)
      } else {
        logEvent('play() rejected', 'element was already replaced')
      }
    })
  }, [logEvent, updateState])

  const pause = useCallback(() => {
    logEvent('pause() called')
    audioRef.current?.pause()
  }, [logEvent])

  const resume = useCallback(() => {
    logEvent('resume() called')
    const a = audioRef.current
    if (!a) {
      logEvent('resume() failed', 'no audio element')
      return
    }
    if (!a.src) {
      logEvent('resume() failed', 'src is empty')
      setError('Cannot resume: no stream loaded')
      return
    }
    a.play().catch((e) => {
      if (audioRef.current === a) {
        logEvent('resume() failed', e?.message)
        setError(`Playback blocked: ${e?.message || 'unknown'}`)
      }
    })
  }, [logEvent])

  const seek = useCallback((t: number) => {
    if (audioRef.current) audioRef.current.currentTime = t
  }, [])

  return { audioRef, playing, currentTime, duration, error, loading, diagnostics, play, pause, resume, seek, setOnEnded }
}

function mediaErrorName(code: number): string {
  switch (code) {
    case 1: return 'ABORTED'
    case 2: return 'NETWORK'
    case 3: return 'DECODE'
    case 4: return 'SRC_NOT_SUPPORTED'
    default: return `UNKNOWN(${code})`
  }
}
