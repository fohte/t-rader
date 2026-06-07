import { useCallback, useEffect, useState } from 'react'

// portfolio_snapshot 用の API が未提供のため localStorage に保存する。
const STORAGE_KEY = 't-rader:cash-balance-jpy'

function readStored(): number {
  if (typeof window === 'undefined') return 0
  const raw = window.localStorage.getItem(STORAGE_KEY)
  if (raw == null) return 0
  const n = Number(raw)
  return Number.isFinite(n) ? n : 0
}

export function useCashBalance(): {
  cash: number
  setCash: (v: number) => void
} {
  const [cash, setCashState] = useState<number>(() => readStored())

  useEffect(() => {
    const onStorage = (e: StorageEvent) => {
      if (e.key === STORAGE_KEY) setCashState(readStored())
    }
    window.addEventListener('storage', onStorage)
    return () => {
      window.removeEventListener('storage', onStorage)
    }
  }, [])

  const setCash = useCallback((v: number) => {
    const next = Number.isFinite(v) && v >= 0 ? v : 0
    window.localStorage.setItem(STORAGE_KEY, String(next))
    setCashState(next)
  }, [])

  return { cash, setCash }
}
