import { useEffect, useRef, useState } from 'react'

// 戦略ホームの「前回開いた時刻」を localStorage に保持する。
// マウント時点の値を snapshot として返し、直後に「いま」で上書きする。
// 同一セッション内で reload しても snapshot は読み取り済みの値を保つので、
// 「前回開いてから N 件」の判定は開いた瞬間に確定する。
export function useLastVisited(strategyId: string): number | null {
  const key = `t-rader:lastVisited:${strategyId}`
  const [snapshot] = useState<number | null>(() => {
    const raw = window.localStorage.getItem(key)
    if (raw == null) return null
    const v = Number(raw)
    return Number.isFinite(v) ? v : null
  })
  const wroteRef = useRef(false)
  useEffect(() => {
    if (wroteRef.current) return
    wroteRef.current = true
    window.localStorage.setItem(key, String(Date.now()))
  }, [key])
  return snapshot
}
