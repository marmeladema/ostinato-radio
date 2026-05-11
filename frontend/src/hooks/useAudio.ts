import { useRef, useState, useCallback } from 'react'

export function useAudio() {
  const audioRef = useRef<HTMLAudioElement | null>(null)
  const [playing, setPlaying] = useState(false)
  const [currentTime, setCurrentTime] = useState(0)
  const [duration, setDuration] = useState(0)

  const init = useCallback(() => {
    if (!audioRef.current) {
      const a = new Audio()
      a.addEventListener('play', () => setPlaying(true))
      a.addEventListener('pause', () => setPlaying(false))
      a.addEventListener('timeupdate', () => setCurrentTime(a.currentTime))
      a.addEventListener('loadedmetadata', () => setDuration(a.duration))
      a.addEventListener('ended', () => setPlaying(false))
      audioRef.current = a
    }
  }, [])

  const play = useCallback((url: string) => {
    init()
    const a = audioRef.current!
    if (a.src !== url) {
      a.src = url
      a.load()
    }
    a.play().catch(() => {})
  }, [init])

  const pause = useCallback(() => {
    audioRef.current?.pause()
  }, [])

  const resume = useCallback(() => {
    audioRef.current?.play().catch(() => {})
  }, [])

  const seek = useCallback((t: number) => {
    if (audioRef.current) audioRef.current.currentTime = t
  }, [])

  return { audioRef, playing, currentTime, duration, play, pause, resume, seek }
}
