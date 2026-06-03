import { createFileRoute } from '@tanstack/react-router'

import { EmptyPlaceholder } from '@/components/strategy-shell/empty-placeholder'

export const Route = createFileRoute('/strategies/$id/settings')({
  component: StrategySettingsPage,
})

function StrategySettingsPage() {
  return (
    <EmptyPlaceholder
      title="戦略設定"
      description="基本情報・ノートタイプ語彙の管理。AGENTS.md / skills / トリガ / 関心ツリー / インジケーター UI は後続 PR で。"
    />
  )
}
