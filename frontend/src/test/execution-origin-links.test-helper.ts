import { screen } from '@testing-library/react'

export function getExecutionOriginLinks(strategyName = '架空戦略') {
  return [
    ...screen.queryAllByRole('link', { name: strategyName }),
    ...screen.queryAllByRole('link', { name: '実行詳細 →' }),
  ]
}
