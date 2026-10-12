import type { components } from '#lib/api/schema.gen'

type PaperAccountStats = components['schemas']['PaperAccountStats']

export function resolvePaperTradingAccountState(
  stats: readonly PaperAccountStats[],
  accountId: string,
  portfolioQuery: { isPending: boolean; isError: boolean },
) {
  const accountStats = stats.find((account) => account.account_id === accountId)
  if (accountStats == null) return { state: 'not-found' } as const
  if (portfolioQuery.isPending) return { state: 'loading' } as const
  if (portfolioQuery.isError) return { state: 'error' } as const
  return { state: 'ready', stats: accountStats } as const
}
