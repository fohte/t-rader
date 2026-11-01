import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'
import { useState } from 'react'

import type { ExecutionPayloadReference } from '#components/strategy-shell/task-execution-tree'

export type TaskRunPayloadRead =
  | { status: 'loading' | 'error' }
  | {
      status: 'ready'
      manifest: {
        status: string
        itemCount: number
        incompleteReason: string | null
      }
      page: {
        data: unknown
        totalCount: number
        nextCursor: number | null
      } | null
    }

export interface TaskRunPayloadPanelProps {
  reference: ExecutionPayloadReference
  payloadRead?: TaskRunPayloadRead
  initialPointerDraft?: string
  onSelectPayload?: (reference: ExecutionPayloadReference) => void
  onNextPayloadPage?: (cursor: number) => void
}

export function TaskRunPayloadPanel({
  reference,
  payloadRead,
  initialPointerDraft,
  onSelectPayload,
  onNextPayloadPage,
}: TaskRunPayloadPanelProps): React.ReactElement {
  const page = payloadRead?.status === 'ready' ? payloadRead.page : null
  const nextCursor = page?.nextCursor
  const [pointerDraft, setPointerDraft] = useState(
    initialPointerDraft ?? reference.pointer,
  )

  return (
    <section
      aria-label="実行 payload"
      className="mt-4 border-t border-border pt-4"
    >
      <h2 className="mb-2 font-mono text-xs font-bold">{reference.label}</h2>
      <p className="mb-2 break-all font-mono text-2xs text-muted-foreground">
        {reference.payloadId} · pointer {reference.pointer || '/'}
      </p>
      <form
        className="mb-3 flex items-end gap-2"
        onSubmit={(event) => {
          event.preventDefault()
          if (onSelectPayload == null) return
          onSelectPayload({ ...reference, pointer: pointerDraft, cursor: 0 })
        }}
      >
        <label className="min-w-0 flex-1 font-mono text-2xs text-muted-foreground-strong">
          JSON Pointer
          <Input
            aria-label="JSON Pointer"
            value={pointerDraft}
            onChange={(event) => {
              setPointerDraft(event.currentTarget.value)
            }}
          />
        </label>
        <Button
          type="submit"
          variant="plain"
          size="sm"
          disabled={
            onSelectPayload == null || pointerDraft === reference.pointer
          }
        >
          pointer を適用
        </Button>
      </form>
      {payloadRead?.status === 'loading' ? (
        <p className="font-mono text-xs text-muted-foreground">
          payload を読み込み中です。
        </p>
      ) : payloadRead?.status === 'error' ? (
        <p className="font-mono text-xs text-destructive">
          payload を取得できませんでした。
        </p>
      ) : payloadRead?.status !== 'ready' ? (
        <p className="font-mono text-xs text-muted-foreground">
          payload を選択してください。
        </p>
      ) : payloadRead.manifest.status !== 'sealed' ? (
        <p className="font-mono text-xs text-muted-foreground">
          {payloadRead.manifest.status === 'staging'
            ? 'payload の記録が完了していません。'
            : 'payload は未完了のため内容を表示できません。'}{' '}
          {payloadRead.manifest.incompleteReason ?? ''}
        </p>
      ) : page == null ? (
        <p className="font-mono text-xs text-muted-foreground">
          payload page がありません。
        </p>
      ) : (
        <>
          <p className="mb-2 font-mono text-2xs text-muted-foreground">
            全 {page.totalCount} 件 · 表示{' '}
            {Array.isArray(page.data) ? page.data.length : 1} 件
          </p>
          <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all font-mono text-2xs">
            {JSON.stringify(page.data, null, 2)}
          </pre>
          {nextCursor != null && onNextPayloadPage != null && (
            <Button
              type="button"
              variant="link"
              size="sm"
              className="mt-2"
              onClick={() => {
                onNextPayloadPage(nextCursor)
              }}
            >
              次のページを表示
            </Button>
          )}
        </>
      )}
    </section>
  )
}
