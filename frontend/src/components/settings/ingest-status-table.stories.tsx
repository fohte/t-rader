import type { Meta, StoryObj } from '@storybook/react-vite'

import { IngestStatusTable } from '#components/settings/ingest-status-table'
import type { components } from '#lib/api/schema.gen'

type IngestJob = components['schemas']['IngestJobStatusResponse']

const jobs: IngestJob[] = [
  {
    job: 'example-daily-import',
    last_run: {
      id: 'run-example-daily',
      status: 'succeeded',
      started_at: '2099-01-02T03:00:00Z',
      finished_at: '2099-01-02T03:04:00Z',
      stats: { rows_written: 1234, days_processed: 5 },
      error: null,
    },
    last_succeeded_at: '2099-01-02T03:04:00Z',
    latest_data_date: '2099-01-01',
    expected_data_date: '2099-01-02',
  },
  {
    job: 'example-reference-import',
    last_run: {
      id: 'run-example-reference',
      status: 'failed',
      started_at: '2099-01-02T04:00:00Z',
      finished_at: '2099-01-02T04:02:00Z',
      stats: null,
      error: '架空の接続エラーです。',
    },
    last_succeeded_at: '2099-01-01T04:02:00Z',
    latest_data_date: null,
    expected_data_date: '2099-01-02',
  },
]

const meta = {
  title: 'Settings/IngestStatusTable',
  component: IngestStatusTable,
  parameters: { layout: 'padded' },
} satisfies Meta<typeof IngestStatusTable>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {
  name: 'shows recent ingest runs and their data dates.',
  args: { jobs },
}

export const Running: Story = {
  name: 'shows a job while its latest run is still running.',
  args: {
    jobs: [
      {
        job: 'example-active-import',
        last_run: {
          id: 'run-example-active',
          status: 'running',
          started_at: '2099-01-02T06:00:00Z',
          finished_at: null,
          stats: null,
          error: null,
        },
        last_succeeded_at: null,
        latest_data_date: null,
        expected_data_date: '2099-01-01',
      },
    ],
  },
}

export const Empty: Story = {
  name: 'shows an empty state when no ingest runs are available.',
  args: { jobs: [] },
}
