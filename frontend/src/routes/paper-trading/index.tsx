import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import { PaperTradingAccountsView } from '#components/paper-trading/accounts-view'
import { PaperTradingPageStateView } from '#components/paper-trading/page-state'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/paper-trading/')({
  component: PaperTradingAccountsPage,
})

function PaperTradingAccountsPage() {
  const statsQuery = $api.useQuery('get', '/api/paper-accounts/stats')
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')

  const strategyNames = useMemo(
    () =>
      Object.fromEntries(
        strategies.map((strategy) => [strategy.id, strategy.name]),
      ),
    [strategies],
  )

  if (statsQuery.isPending) {
    return <PaperTradingPageStateView state="loading" />
  }
  if (statsQuery.isError) {
    return <PaperTradingPageStateView state="error" />
  }

  return (
    <PaperTradingAccountsView
      stats={statsQuery.data}
      strategyNames={strategyNames}
    />
  )
}
