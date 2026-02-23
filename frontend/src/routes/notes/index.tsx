import { createFileRoute } from '@tanstack/react-router'
import { FileText } from 'lucide-react'

import { PlaceholderPage } from '@/components/placeholder-page'

export const Route = createFileRoute('/notes/')({
  component: NotesPage,
})

function NotesPage() {
  return (
    <PlaceholderPage
      title="ノート"
      description="日次・週次の振り返りや分析メモを記録できます"
      icon={FileText}
    />
  )
}
