import { Result } from 'neverthrow'
import { useCallback, useEffect, useState } from 'react'

// portfolio_snapshot 用の API が未提供のため localStorage に保存する。
const STORAGE_KEY = 't-rader:cash-balance-jpy'

// private browsing / ストレージ無効環境では SecurityError が飛ぶことがある。
const readRawItem = Result.fromThrowable((key: string) =>
  window.localStorage.getItem(key),
)
// QuotaExceededError 等で書き込みに失敗することがある。
const writeRawItem = Result.fromThrowable((key: string, value: string) => {
  window.localStorage.setItem(key, value)
})

function readStored(): number {
  if (typeof window === 'undefined') return 0
  const result = readRawItem(STORAGE_KEY)
  if (result.isErr()) return 0
  const raw = result.value
  if (raw == null) return 0
  const n = Number(raw)
  return Number.isFinite(n) && n >= 0 ? n : 0
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
    // 書き込みに失敗してもメモリ上の状態は更新する。
    const writeResult = writeRawItem(STORAGE_KEY, String(next))
    if (writeResult.isErr()) {
      // noop
    }
    setCashState(next)
  }, [])

  return { cash, setCash }
}
