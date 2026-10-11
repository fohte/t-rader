import { createFileRoute } from '@tanstack/react-router'
import { useMemo } from 'react'

import { PaperTradingAccountDetailView } from '#components/paper-trading/account-detail-view'
import { PaperTradingPageStateView } from '#components/paper-trading/page-state'
import { $api } from '#lib/api/client'
import { resolvePaperTradingAccountState } from '#lib/paper-trading-account-state'

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

  if (statsQuery.isPending) {
    return <PaperTradingPageStateView state="loading" />
  }
  if (statsQuery.isError) {
    return <PaperTradingPageStateView state="error" />
  }

  const accountState = resolvePaperTradingAccountState(
    statsQuery.data,
    accountId,
    portfolioQuery,
  )
  if (accountState.state !== 'ready') {
    return <PaperTradingPageStateView state={accountState.state} />
  }
  const portfolio = portfolioQuery.data
  if (portfolio == null) {
    return <PaperTradingPageStateView state="error" />
  }

  return (
    <PaperTradingAccountDetailView
      stats={accountState.stats}
      portfolio={portfolio}
      strategyName={
        strategyNames[portfolio.strategy_id] ?? portfolio.strategy_id
      }
      stockNames={stockNames}
    />
  )
}
