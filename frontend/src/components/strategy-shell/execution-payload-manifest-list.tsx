import { Button } from '@fohte/ui/button'

export interface ExecutionPayloadManifestSummary {
  id: string
  kind: string
  source: string
  status: string
  itemCount: number
  byteCount: number
  digest: string | null
  incompleteReason: string | null
  createdAt: string
}

export type ExecutionPayloadManifestPageState =
  | { status: 'loading' }
  | { status: 'error' }
  | {
      status: 'ready'
      items: ExecutionPayloadManifestSummary[]
      totalCount: number
      nextCursor: string | null
      selectedPayloadId: string | null
      hasPreviousPage?: boolean
    }

interface ExecutionPayloadManifestListProps {
  state: ExecutionPayloadManifestPageState
  onSelectManifest?: (manifest: ExecutionPayloadManifestSummary) => void
  onNextPage?: (cursor: string) => void
  onPreviousPage?: () => void
}

function statusLabel(status: string): string {
  switch (status) {
    case 'staging':
      return '記録中'
    case 'sealed':
      return '記録完了'
    case 'incomplete':
      return '未完了'
    default:
      return status
  }
}

function kindLabel(kind: string): string {
  switch (kind) {
    case 'input':
      return '入力'
    case 'output':
      return '出力'
    default:
      return kind
  }
}

export function ExecutionPayloadManifestList({
  state,
  onSelectManifest,
  onNextPage,
  onPreviousPage,
}: ExecutionPayloadManifestListProps): React.ReactElement {
  const nextCursor = state.status === 'ready' ? state.nextCursor : null

  return (
    <section
      aria-label="実行 payload 記録"
      className="mt-4 border-t border-border pt-4"
    >
      <h2 className="mb-2 font-mono text-xs font-bold">実行 payload 記録</h2>
      {state.status === 'loading' ? (
        <p className="font-mono text-xs text-muted-foreground">
          payload 記録を読み込み中です。
        </p>
      ) : state.status === 'error' ? (
        <p className="font-mono text-xs text-destructive">
          payload 記録を取得できませんでした。
        </p>
      ) : (
        <>
          <p className="mb-2 font-mono text-2xs text-muted-foreground">
            取得時点の現在総数 {state.totalCount} 件 · 表示 {state.items.length}{' '}
            件
          </p>
          <p className="mb-2 font-mono text-2xs text-muted-foreground">
            一覧は live です。追加分は先頭ページを再読すると確認できます。
          </p>
          {state.items.length === 0 ? (
            <p className="font-mono text-xs text-muted-foreground">
              payload 記録がありません。
            </p>
          ) : (
            <ul className="space-y-2">
              {state.items.map((item) => (
                <li
                  key={item.id}
                  className="border border-border p-2 font-mono text-2xs"
                >
                  <p className="break-all text-foreground">
                    {item.id} · {kindLabel(item.kind)} ·{' '}
                    {statusLabel(item.status)}
                  </p>
                  <p className="mt-1 break-all text-muted-foreground">
                    {item.source} · {item.itemCount} 件 ·{' '}
                    {item.byteCount.toLocaleString('ja-JP')} bytes ·{' '}
                    {item.createdAt}
                  </p>
                  {item.digest != null && (
                    <p className="mt-1 break-all text-muted-foreground">
                      SHA-256 {item.digest}
                    </p>
                  )}
                  {item.incompleteReason != null && (
                    <p className="mt-1 break-all text-muted-foreground">
                      理由: {item.incompleteReason}
                    </p>
                  )}
                  {onSelectManifest != null && (
                    <Button
                      type="button"
                      variant="link"
                      size="sm"
                      className="mt-1 h-auto"
                      aria-current={
                        state.selectedPayloadId === item.id ? 'true' : undefined
                      }
                      onClick={() => {
                        onSelectManifest(item)
                      }}
                    >
                      {state.selectedPayloadId === item.id
                        ? '選択中'
                        : 'payload を選択'}
                    </Button>
                  )}
                </li>
              ))}
            </ul>
          )}
          {state.hasPreviousPage === true && onPreviousPage != null && (
            <Button
              type="button"
              variant="link"
              size="sm"
              className="mt-2"
              onClick={() => {
                onPreviousPage()
              }}
            >
              前の記録を表示
            </Button>
          )}
          {nextCursor != null && onNextPage != null && (
            <Button
              type="button"
              variant="link"
              size="sm"
              className="mt-2"
              onClick={() => {
                onNextPage(nextCursor)
              }}
            >
              次の記録を表示
            </Button>
          )}
        </>
      )}
    </section>
  )
}
