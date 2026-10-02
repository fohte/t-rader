import { createFileRoute } from '@tanstack/react-router'

import { IngestStatusPageHeader } from '#components/settings/ingest-status-page-header'
import { IngestStatusPageState } from '#components/settings/ingest-status-page-state'
import { IngestStatusTable } from '#components/settings/ingest-status-table'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/settings/ingest-status')({
  component: IngestStatusSettingsPage,
})

function IngestStatusSettingsPage() {
  const { data, isPending, isError } = $api.useQuery(
    'get',
    '/api/ingest-status',
  )

  return (
    <div className="space-y-5">
      <IngestStatusPageHeader />

      {isPending ? (
        <IngestStatusPageState state="loading" />
      ) : isError ? (
        <IngestStatusPageState state="error" />
      ) : (
        <IngestStatusTable jobs={data.jobs} />
      )}
    </div>
  )
}
