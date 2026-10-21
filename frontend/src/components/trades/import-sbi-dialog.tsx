import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'
import { ResultAsync } from 'neverthrow'
import { useState } from 'react'

import {
  ImportSbiReview,
  type ImportSbiRow,
} from '#components/trades/import-sbi-review'
import { useInvalidateTrades } from '#components/trades/use-invalidate-trades'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '#components/ui/dialog'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type SbiPreviewResponse = components['schemas']['SbiPreviewResponse']
type SbiCommitResponse = components['schemas']['SbiCommitResponse']

type Phase = 'select' | 'previewing' | 'review' | 'committing' | 'done'

export function ImportSbiDialog({
  open,
  onOpenChange,
  strategies,
}: {
  open: boolean
  onOpenChange: (v: boolean) => void
  strategies: Strategy[]
}) {
  const invalidateTrades = useInvalidateTrades()
  const [phase, setPhase] = useState<Phase>('select')
  const [error, setError] = useState<string | null>(null)
  const [rows, setRows] = useState<ImportSbiRow[]>([])
  const [issues, setIssues] = useState<SbiPreviewResponse['issues']>([])
  const [bulkStrategy, setBulkStrategy] = useState<string>('')
  const [summary, setSummary] = useState<SbiCommitResponse | null>(null)

  function reset() {
    setPhase('select')
    setError(null)
    setRows([])
    setIssues([])
    setBulkStrategy('')
    setSummary(null)
  }

  function handleOpenChange(v: boolean) {
    if (!v) reset()
    onOpenChange(v)
  }

  async function handleFile(file: File) {
    setPhase('previewing')
    setError(null)

    // openapi-fetch は text/csv の raw body 送信を素直に表現できないので fetch を直接使う
    const result = await ResultAsync.fromPromise(
      (async () => {
        const buf = await file.arrayBuffer()
        return fetch('/api/imports/sbi/preview', {
          method: 'POST',
          headers: { 'content-type': 'text/csv' },
          body: buf,
        })
      })(),
      (e) => (e instanceof Error ? e.message : String(e)),
    )
    if (result.isErr()) {
      setError(result.error)
      setPhase('select')
      return
    }
    const httpRes = result.value

    if (!httpRes.ok) {
      const errBody: unknown = await httpRes.json().catch(() => null)
      const message =
        errBody != null &&
        typeof errBody === 'object' &&
        'error' in errBody &&
        typeof errBody.error === 'string'
          ? errBody.error
          : 'CSV の解析に失敗しました'
      setError(message)
      setPhase('select')
      return
    }
    // eslint-disable-next-line @typescript-eslint/no-unsafe-type-assertion -- 自前 API でスキーマ一致を信頼するため、unknown→具象型は narrow 検証なしで通す
    const data = (await httpRes.json()) as SbiPreviewResponse
    const defaultStrategy = strategies[0]?.id ?? ''
    setRows(
      data.rows.map((r) => ({
        ...r,
        strategyId: defaultStrategy,
        excluded: r.is_duplicate,
      })),
    )
    setIssues(data.issues)
    setBulkStrategy(defaultStrategy)
    setPhase('review')
  }

  function applyBulkStrategy(id: string) {
    setBulkStrategy(id)
    if (id === '') return
    setRows((prev) => prev.map((r) => ({ ...r, strategyId: id })))
  }

  function updateRow(
    idx: number,
    patch: Partial<Pick<ImportSbiRow, 'strategyId' | 'excluded'>>,
  ) {
    setRows((prev) => prev.map((r, i) => (i === idx ? { ...r, ...patch } : r)))
  }

  async function handleCommit() {
    const target = rows.filter((r) => !r.excluded && r.strategyId !== '')
    if (target.length === 0) {
      setError('取込対象の行がありません')
      return
    }
    setPhase('committing')
    setError(null)
    const res = await fetchClient.POST('/api/imports/sbi/commit', {
      body: {
        rows: target.map((r) => ({
          strategy_id: r.strategyId,
          date: r.date,
          symbol: r.symbol,
          stock_name: r.stock_name,
          side: r.side,
          qty: r.qty,
          price: r.price,
          fee: r.fee,
        })),
      },
    })
    if (res.data === undefined) {
      // openapi-fetch の discriminated union により data===undefined の分岐では
      // res.error が必ず ErrorResponse になる。fetch 失敗は throw されて catch される。
      setError(res.error.error)
      setPhase('review')
      return
    }
    setSummary(res.data)
    setPhase('done')
    invalidateTrades()
  }

  const importableCount = rows.filter(
    (r) => !r.excluded && r.strategyId !== '',
  ).length

  return (
    <Dialog open={open} onOpenChange={handleOpenChange}>
      <DialogContent className="sm:max-w-4xl">
        <DialogHeader>
          <DialogTitle>SBI CSV 取込</DialogTitle>
          <DialogDescription>
            SBI 証券「口座管理 → 取引履歴 → 国内株式」からダウンロードした CSV
            (Shift_JIS) をアップロードします。MVP では国内株現物のみ対象です。
          </DialogDescription>
        </DialogHeader>

        {error != null && (
          <p className="border border-primary bg-primary/10 px-3 py-2 text-sm text-primary">
            {error}
          </p>
        )}

        {phase === 'select' || phase === 'previewing' ? (
          <div className="flex flex-col gap-3 py-2">
            <label className="text-sm text-muted-foreground-strong">
              CSV ファイルを選択
              <Input
                type="file"
                accept=".csv,text/csv"
                disabled={phase === 'previewing'}
                onChange={(e) => {
                  const f = e.target.files?.[0]
                  if (f != null) void handleFile(f)
                }}
                className="mt-1 block w-full"
              />
            </label>
            {phase === 'previewing' && (
              <p className="text-xs text-muted-foreground">解析中…</p>
            )}
          </div>
        ) : null}

        {(phase === 'review' || phase === 'committing') && (
          <ImportSbiReview
            rows={rows}
            strategies={strategies}
            issues={issues}
            bulkStrategy={bulkStrategy}
            importableCount={importableCount}
            onBulkStrategyChange={applyBulkStrategy}
            onRowChange={updateRow}
          />
        )}

        {phase === 'done' && summary != null && (
          <div className="space-y-2 py-2 text-sm">
            <p>取込が完了しました。</p>
            <ul className="list-disc pl-5">
              <li>追加: {summary.imported_count} 件</li>
              <li>重複 skip: {summary.skipped_count} 件</li>
            </ul>
          </div>
        )}

        <DialogFooter>
          {phase === 'review' && (
            <Button
              type="button"
              onClick={() => {
                void handleCommit()
              }}
              disabled={importableCount === 0}
            >
              取込実行 ({importableCount} 件)
            </Button>
          )}
          {phase === 'committing' && (
            <Button type="button" disabled>
              取込中…
            </Button>
          )}
          {phase === 'done' && (
            <Button
              type="button"
              onClick={() => {
                handleOpenChange(false)
              }}
            >
              閉じる
            </Button>
          )}
          {(phase === 'select' || phase === 'previewing') && (
            <Button
              type="button"
              variant="outline"
              onClick={() => {
                handleOpenChange(false)
              }}
            >
              キャンセル
            </Button>
          )}
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
