import { $api } from '#lib/api/client'

export type RefKind = 'stock' | 'indicator' | 'sector' | 'theme'

export interface RefEntity {
  kind: RefKind
  name: string
  sub?: string
}

export const REF_KIND_JP: Record<RefKind, string> = {
  stock: '銘柄',
  indicator: '指標',
  sector: 'セクター',
  theme: 'テーマ',
}

function isRefKind(s: string): s is RefKind {
  return s in REF_KIND_JP
}

// backend の /api/refs/resolve で解決できなかった token (未知の id、未読み込み中含む)
// 向けのフォールバック表示。prefix を落として残りをそのまま name とする。
function fallbackRef(token: string): RefEntity {
  const i = token.indexOf(':')
  if (i < 0) return { kind: 'stock', name: token }
  const prefix = token.slice(0, i)
  const kind: RefKind = isRefKind(prefix) ? prefix : 'stock'
  return { kind, name: token.slice(i + 1) }
}

// `stock:7203` のような `[[kind:id]]` token を backend の解決 API で表示名に変換する。
// options.enabled が false の間は token を取得しない (呼び出し元の他データに
// token 自体が依存し、まだ準備できていない場合に使う)。
export function useResolveRef(
  token: string,
  options?: { enabled?: boolean },
): RefEntity {
  const enabled = options?.enabled ?? true
  const { data } = $api.useQuery(
    'get',
    '/api/refs/resolve',
    { params: { query: { link: token } } },
    { enabled },
  )
  const resolved = data?.[0]
  if (!enabled || resolved?.name == null || !isRefKind(resolved.kind)) {
    return fallbackRef(token)
  }
  return {
    kind: resolved.kind,
    name: resolved.name,
    sub: resolved.kind === 'stock' ? resolved.id : undefined,
  }
}
