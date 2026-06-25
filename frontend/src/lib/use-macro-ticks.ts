import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

export type MacroTick = components['schemas']['MacroTick']
export type MacroTicksResponse = components['schemas']['MacroTicksResponse']

const POLL_INTERVAL_MS = 60 * 1000

export interface UseMacroTicksResult {
  ticks: MacroTick[] | null
  staleSince: string | null
  isPending: boolean
}

/**
 * `/api/macro/ticks` を 60s 間隔で polling する hook
 *
 * - `ticks === null`: 24h 以上更新失敗が続いており N/A 表示する
 * - `staleSince !== null && ticks !== null`: 値はあるが古い (バッジ表示)
 */
export function useMacroTicks(): UseMacroTicksResult {
  const { data, isPending } = $api.useQuery(
    'get',
    '/api/macro/ticks',
    {},
    { refetchInterval: POLL_INTERVAL_MS },
  )

  return {
    ticks: data?.ticks ?? null,
    staleSince: data?.stale_since ?? null,
    isPending,
  }
}
