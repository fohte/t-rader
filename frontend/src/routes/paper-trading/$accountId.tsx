import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import { PaperTradingAccountDetailView } from '#components/paper-trading/account-detail-view'
import { PaperTradingPageStateView } from '#components/paper-trading/page-state'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/paper-trading/$accountId')({
  component: PaperTradingAccountPage,
})

function PaperTradingAccountPage() {
  const { accountId } = Route.useParams()
  const portfolioQuery = $api.useQuery(
    'get',
    '/api/paper-accounts/{id}/portfolio',
    { params: { path: { id: accountId } } },
  )
  const statsQuery = $api.useQuery('get', '/api/paper-accounts/stats')
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: stocks = [] } = $api.useQuery('get', '/api/refs/stocks')

  const strategyNames = useMemo(
    () =>
      Object.fromEntries(
        strategies.map((strategy) => [strategy.id, strategy.name]),
      ),
    [strategies],
  )
  const stockNames = useMemo(
    () => Object.fromEntries(stocks.map((stock) => [stock.id, stock.name])),
    [stocks],
  )

  if (portfolioQuery.isPending || statsQuery.isPending) {
    return <PaperTradingPageStateView state="loading" />
  }
  if (portfolioQuery.isError || statsQuery.isError) {
    return <PaperTradingPageStateView state="error" />
  }

  const stats = statsQuery.data.find(
    (account) => account.account_id === accountId,
  )
  if (stats == null) {
    return <PaperTradingPageStateView state="not-found" />
  }

  return (
    <PaperTradingAccountDetailView
      stats={stats}
      portfolio={portfolioQuery.data}
      strategyName={
        strategyNames[portfolioQuery.data.strategy_id] ??
        portfolioQuery.data.strategy_id
      }
      stockNames={stockNames}
    />
  )
}
