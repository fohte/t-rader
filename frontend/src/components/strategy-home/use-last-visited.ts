import { useEffect, useRef, useState } from 'react'

function readStoredTimestamp(key: string): number | null {
  const raw = window.localStorage.getItem(key)
  if (raw == null) return null
  const v = Number(raw)
  return Number.isFinite(v) ? v : null
}

// 戦略ホームの「前回開いた時刻」を localStorage に保持する。
// マウントまたは strategyId 切り替えの瞬間に snapshot を読み取り、直後に現在時刻で上書きする。
// 同一セッション内で reload しても snapshot は読み取り済みの値を保つ。
export function useLastVisited(strategyId: string): number | null {
  const key = `t-rader:lastVisited:${strategyId}`
  const [snapshot, setSnapshot] = useState<number | null>(() =>
    readStoredTimestamp(key),
  )
  const prevKeyRef = useRef(key)
  if (prevKeyRef.current !== key) {
    prevKeyRef.current = key
    setSnapshot(readStoredTimestamp(key))
  }
  const writtenKeyRef = useRef<string | null>(null)
  useEffect(() => {
    if (writtenKeyRef.current === key) return
    writtenKeyRef.current = key
    window.localStorage.setItem(key, String(Date.now()))
  }, [key])
  return snapshot
}
