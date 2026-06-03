import { useMatches } from '@tanstack/react-router'

// URL パラメータから現在の戦略 ID を抽出する。
// 戦略コンテキスト外 (戦略一覧 / portfolio など) では undefined を返す。
export function useCurrentStrategyId(): string | undefined {
  const matches = useMatches()
  for (const m of matches) {
    const params = m.params as Record<string, unknown>
    const id = params['id']
    if (typeof id === 'string') return id
  }
  return undefined
}
