import { useEffect, useRef, useState } from 'react'

// 戦略ホームの「前回開いた時刻」を localStorage に保持する。
// マウント時の値を返し、マウント直後に「いま」で更新する。
export function useLastVisited(strategyId: string): number | null {
  const key = `t-rader:lastVisited:${strategyId}`
  const [snapshot] = useState<number | null>(() => {
    if (typeof window === 'undefined') return null
    const raw = window.localStorage.getItem(key)
    if (raw == null) return null
    const v = Number(raw)
    return Number.isFinite(v) ? v : null
  })
  const wroteRef = useRef(false)
  useEffect(() => {
    if (wroteRef.current) return
    wroteRef.current = true
    if (typeof window === 'undefined') return
    window.localStorage.setItem(key, String(Date.now()))
  }, [key])
  return snapshot
}
