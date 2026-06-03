import { useParams } from '@tanstack/react-router'

// `/strategies/$id` 配下のいずれかのルートにマッチしているときの id を返す。
// 他ルートでは undefined。strict: false で全ルートの params union を引く。
export function useCurrentStrategyId(): string | undefined {
  return useParams({ strict: false }).id
}
