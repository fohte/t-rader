import { $api } from '@/lib/api/client'
import { formatRelative } from '@/lib/note-utils'

interface HistoryPanelProps {
  noteId: string
}

const OP_LABEL: Record<string, string> = {
  create: '作成',
  update: '更新',
  approve: '承認',
  reject: '却下',
  delete: '削除',
}

export function HistoryPanel({ noteId }: HistoryPanelProps) {
  const { data: history, isPending } = $api.useQuery('get', '/api/history', {
    params: { query: { target_kind: 'note', target_id: noteId, limit: 50 } },
  })
  const rows = history ?? []

  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <header className="border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          変更履歴
        </h3>
      </header>
      {isPending ? (
        <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
          読み込み中…
        </div>
      ) : rows.length === 0 ? (
        <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
          —
        </div>
      ) : (
        <div className="divide-y divide-[color:var(--color-hairline)]">
          {rows.map((h) => {
            const isLLM = h.actor_kind === 'llm'
            return (
              <div
                key={h.id}
                className="grid grid-cols-[auto_auto_1fr] items-baseline gap-2 px-3.5 py-2 font-mono text-[11px]"
              >
                <span className="text-[color:var(--color-text-tertiary)]">
                  {formatRelative(h.created_at)}
                </span>
                <span
                  className={`border px-1 text-[10px] ${
                    isLLM
                      ? 'border-[color:var(--color-accent-strategy)] text-[color:var(--color-accent-strategy)]'
                      : 'border-[color:var(--color-text-tertiary)] text-[color:var(--color-text-primary)]'
                  }`}
                >
                  {isLLM ? 'LLM' : h.actor_label}
                </span>
                <span className="text-[color:var(--color-text-secondary)]">
                  {h.summary != null && h.summary !== ''
                    ? h.summary
                    : (OP_LABEL[h.op] ?? h.op)}
                </span>
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}
