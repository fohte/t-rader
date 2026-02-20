import { useCallback, useState } from 'react'

/**
 * instrument_id → name のマッピングをセッション中保持するフック。
 * WatchlistItem レスポンスに name が含まれないため、追加時の入力値をクライアント側で管理する。
 */
export function useInstrumentNames() {
  const [names, setNames] = useState<Map<string, string>>(new Map())

  const registerName = useCallback((instrumentId: string, name: string) => {
    setNames((prev) => new Map(prev).set(instrumentId, name))
  }, [])

  return { names, registerName }
}
