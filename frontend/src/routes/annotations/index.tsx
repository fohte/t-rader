import { createFileRoute } from '@tanstack/react-router'

import { AnnotationsList } from '#components/annotations/annotations-list'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/annotations/')({
  component: AnnotationsPage,
})

function AnnotationsPage() {
  const { data: annotations, isPending } = $api.useQuery(
    'get',
    '/api/annotations',
  )

  return (
    <div className="font-sans text-foreground">
      <div className="mb-6 flex flex-wrap items-center justify-between gap-3">
        <h1 className="text-2xl font-bold tracking-tight">
          <span className="font-mono font-bold text-primary">&gt;</span>{' '}
          アノテーション
        </h1>
      </div>

      {isPending ? (
        <div className="space-y-2">
          <Skeleton className="h-14 w-full" />
          <Skeleton className="h-14 w-full" />
          <Skeleton className="h-14 w-full" />
        </div>
      ) : (
        <AnnotationsList annotations={annotations ?? []} />
      )}
    </div>
  )
}
