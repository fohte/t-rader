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

function fallbackRef(token: string): RefEntity {
  const i = token.indexOf(':')
  if (i < 0) return { kind: 'stock', name: token }
  const prefix = token.slice(0, i)
  const kind: RefKind = isRefKind(prefix) ? prefix : 'stock'
  return { kind, name: token.slice(i + 1) }
}

/**
 * `[[kind:id]]` 形式の参照トークンから表示エンティティを解決する。
 * 未解決 (backend が name を持たない)・読み込み中・options.enabled=false の
 * いずれでも fallbackRef (token から機械的に組み立てた表示名) を返す。
 */
export function useResolveRef(
  token: string,
  options?: { enabled?: boolean },
): RefEntity {
  const enabled = options?.enabled ?? true
  const { data, isError, error } = $api.useQuery(
    'get',
    '/api/refs/resolve',
    { params: { query: { link: token } } },
    { enabled },
  )
  if (isError) {
    console.error('ref resolve failed', token, error)
  }
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
