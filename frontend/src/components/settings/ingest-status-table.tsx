import type { components } from '#lib/api/schema.gen'
import {
  formatIngestDate,
  formatIngestDateTime,
  formatIngestStats,
} from '#lib/format-ingest-status'

type IngestJob = components['schemas']['IngestJobStatusResponse']

export interface IngestStatusTableProps {
  jobs: IngestJob[]
}

const RUN_STATUS_LABELS: Record<string, string> = {
  running: '実行中',
  succeeded: '成功',
  failed: '失敗',
}

export function IngestStatusTable({ jobs }: IngestStatusTableProps) {
  return (
    <div className="space-y-6">
      <section className="space-y-2">
        <h2 className="font-mono text-sm font-semibold">取り込みジョブ</h2>
        {jobs.length === 0 ? (
          <p className="border border-dashed border-border p-5 text-center font-mono text-xs text-muted-foreground">
            表示できるジョブがありません。
          </p>
        ) : (
          <div className="overflow-x-auto border border-border bg-card">
            <table className="w-full min-w-240 font-mono text-xs">
              <thead className="bg-surface-strong text-2xs uppercase tracking-wider text-muted-foreground">
                <tr>
                  <th className="px-3 py-2 text-left font-normal">ジョブ</th>
                  <th className="px-3 py-2 text-left font-normal">状態</th>
                  <th className="px-3 py-2 text-left font-normal">最終成功</th>
                  <th className="px-3 py-2 text-left font-normal">最終実行</th>
                  <th className="px-3 py-2 text-left font-normal">件数</th>
                  <th className="px-3 py-2 text-left font-normal">エラー</th>
                  <th className="px-3 py-2 text-left font-normal">
                    最新データ日
                  </th>
                </tr>
              </thead>
              <tbody>
                {jobs.map((job) => (
                  <tr key={job.job} className="border-t border-border">
                    <td className="px-3 py-2 text-foreground">{job.job}</td>
                    <td
                      className={`px-3 py-2 ${statusClass(job.last_run?.status)}`}
                    >
                      {job.last_run == null
                        ? '未実行'
                        : (RUN_STATUS_LABELS[job.last_run.status] ??
                          job.last_run.status)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-muted-foreground-strong">
                      {formatIngestDateTime(job.last_succeeded_at)}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-muted-foreground-strong">
                      {job.last_run == null ? (
                        '—'
                      ) : (
                        <div className="space-y-1">
                          <div>
                            開始 {formatIngestDateTime(job.last_run.started_at)}
                          </div>
                          <div>
                            終了{' '}
                            {formatIngestDateTime(job.last_run.finished_at)}
                          </div>
                        </div>
                      )}
                    </td>
                    <td className="max-w-80 break-all px-3 py-2 text-muted-foreground-strong">
                      {formatIngestStats(job.last_run?.stats)}
                    </td>
                    <td className="max-w-100 break-words px-3 py-2 text-primary">
                      {job.last_run?.error ?? '—'}
                    </td>
                    <td className="whitespace-nowrap px-3 py-2 text-muted-foreground-strong">
                      {formatIngestDate(job.latest_data_date)}
                      {job.expected_data_date != null && (
                        <div className="text-2xs text-muted-foreground">
                          期待 {formatIngestDate(job.expected_data_date)}
                        </div>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </section>
    </div>
  )
}

function statusClass(status: string | undefined): string {
  if (status === 'failed') return 'text-primary'
  if (status === 'succeeded') return 'text-up'
  return 'text-muted-foreground-strong'
}
