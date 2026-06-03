import { createFileRoute } from '@tanstack/react-router'

import { EmptyPlaceholder } from '@/components/strategy-shell/empty-placeholder'

export const Route = createFileRoute('/strategies/$id/annotations/$annoId')({
  component: AnnotationDetailPage,
})

function AnnotationDetailPage() {
  const { annoId } = Route.useParams()
  return (
    <EmptyPlaceholder
      title={`アノテーション ${annoId}`}
      description="対象データ範囲 + text + コメント + status を表示予定。"
    />
  )
}
