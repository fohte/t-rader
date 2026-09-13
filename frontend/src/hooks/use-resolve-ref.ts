import { $api } from '#lib/api/client'
import { REF_KIND_JP, type RefKind } from '#lib/strategy-mock'

function isRefKind(value: string): value is RefKind {
  return value in REF_KIND_JP
}

export interface ResolvedRef {
  kind: RefKind
  id: string
  name: string | null
}

/**
 * `kind:id` 形式の参照 token を GET /api/refs/resolve で解決する。
 * 不正な形式や未知の kind の token は API を呼ばず未解決 (name: null) として扱う。
 */
export function useResolveRef(token: string): ResolvedRef {
  const i = token.indexOf(':')
  const prefix = i < 0 ? '' : token.slice(0, i)
  const parsedId = i < 0 ? token : token.slice(i + 1)
  const kind: RefKind = isRefKind(prefix) ? prefix : 'stock'
  const canResolve = isRefKind(prefix) && parsedId !== ''

  const { data } = $api.useQuery(
    'get',
    '/api/refs/resolve',
    { params: { query: { link: token } } },
    { enabled: canResolve },
  )

  // 別名で解決できた場合、API は正規の id を返す (トークンの id とは異なりうる)
  const resolution = data?.[0]
  return {
    kind,
    id: resolution?.id ?? parsedId,
    name: resolution?.name ?? null,
  }
}
