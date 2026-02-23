import { createFileRoute } from '@tanstack/react-router'
import { History } from 'lucide-react'

import { PlaceholderPage } from '@/components/placeholder-page'

export const Route = createFileRoute('/trades')({
  component: TradesPage,
})

function TradesPage() {
  return (
    <PlaceholderPage
      title="トレード履歴"
      description="売買記録の一覧と振り返りができます"
      icon={History}
    />
  )
}
