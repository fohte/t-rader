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
    worker_jobs: [
      {
        id: 101,
        task_identifier: 'example-news-import',
        state: 'waiting',
        queue_name: 'example-queue',
        run_at: '2099-01-02T05:00:00Z',
        attempts: 0,
        max_attempts: 3,
        last_error: null,
      },
    ],
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
    worker_jobs: [
      {
        id: 102,
        task_identifier: 'example-reference-import',
        state: 'failed',
        queue_name: null,
        run_at: '2099-01-02T04:02:00Z',
        attempts: 3,
        max_attempts: 3,
        last_error: '架空の worker エラーです。',
      },
    ],
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
  name: 'shows recent ingest runs and queued or failed worker jobs.',
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
        worker_jobs: [],
      },
    ],
  },
}

export const Empty: Story = {
  name: 'shows empty states when no ingest or worker jobs are available.',
  args: { jobs: [] },
}
