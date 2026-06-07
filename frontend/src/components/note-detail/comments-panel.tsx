import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useRef, useState } from 'react'

import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'
import { formatRelative } from '@/lib/note-utils'

type Comment = components['schemas']['Comment']

interface CommentsPanelProps {
  noteId: string
  // 親で選択中の引用テキスト。あれば投稿時に先頭に埋め込む。
  pendingQuote: string | null
  onConsumeQuote: () => void
}

export function CommentsPanel({
  noteId,
  pendingQuote,
  onConsumeQuote,
}: CommentsPanelProps) {
  const queryClient = useQueryClient()
  const { data: comments, isPending } = $api.useQuery('get', '/api/comments', {
    params: { query: { target_kind: 'note', target_id: noteId } },
  })
  const create = $api.useMutation('post', '/api/comments')
  const [draft, setDraft] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (pendingQuote != null && pendingQuote !== '') inputRef.current?.focus()
  }, [pendingQuote])

  const list = comments ?? []
  const topLevel = list.filter((c) => c.parent_id == null)
  const repliesByParent: Record<string, Comment[]> = {}
  for (const c of list) {
    if (c.parent_id != null) {
      ;(repliesByParent[c.parent_id] ??= []).push(c)
    }
  }
  const anchoredCount = topLevel.filter((c) => c.body.startsWith('> ')).length

  const submit = (): void => {
    const txt = draft.trim()
    if (txt === '' || create.isPending) return
    const body =
      pendingQuote != null && pendingQuote !== ''
        ? `${pendingQuote
            .split('\n')
            .map((l) => `> ${l}`)
            .join('\n')}\n\n${txt}`
        : txt
    create.mutate(
      {
        body: {
          target_kind: 'note',
          target_id: noteId,
          body,
        },
      },
      {
        onSuccess: () => {
          void queryClient.invalidateQueries({
            queryKey: $api.queryOptions('get', '/api/comments', {
              params: { query: { target_kind: 'note', target_id: noteId } },
            }).queryKey,
          })
          setDraft('')
          onConsumeQuote()
        },
      },
    )
  }

  return (
    <section className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <header className="flex items-center justify-between border-b border-[color:var(--color-hairline)] px-3.5 py-2">
        <h3 className="font-mono text-[12px] font-bold uppercase tracking-wider text-[color:var(--color-text-primary)]">
          コメント
        </h3>
        <span className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
          {list.length}
          {anchoredCount > 0 && <> · {anchoredCount} 箇所</>}
        </span>
      </header>
      <div className="divide-y divide-[color:var(--color-hairline)]">
        {isPending ? (
          <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
            読み込み中…
          </div>
        ) : topLevel.length === 0 ? (
          <div className="px-3.5 py-3 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
            まだコメントはありません。
          </div>
        ) : (
          topLevel.map((c) => (
            <CommentRow
              key={c.id}
              comment={c}
              replies={repliesByParent[c.id] ?? []}
            />
          ))
        )}
        <div className="px-3.5 py-2 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
          <span className="text-[color:var(--color-accent-strategy)]">▚</span>{' '}
          本文をドラッグして選択 →
          「コメント」でその箇所にスレッドを付けられます。
        </div>
      </div>
      <div className="space-y-2 border-t border-[color:var(--color-hairline)] px-3.5 py-3">
        {pendingQuote != null && pendingQuote !== '' && (
          <div className="flex items-start gap-2 border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-2 py-1.5 text-[11px] text-[color:var(--color-text-secondary)]">
            <span className="font-mono text-[color:var(--color-accent-strategy)]">
              ”
            </span>
            <span className="line-clamp-2 flex-1 italic">{pendingQuote}</span>
            <button
              type="button"
              onClick={onConsumeQuote}
              className="font-mono text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)]"
              title="引用を外す"
            >
              ✕
            </button>
          </div>
        )}
        <div className="flex gap-2">
          <input
            ref={inputRef}
            value={draft}
            onChange={(e) => {
              setDraft(e.target.value)
            }}
            onKeyDown={(e) => {
              if (e.key === 'Enter' && !e.nativeEvent.isComposing) submit()
            }}
            placeholder={
              pendingQuote != null && pendingQuote !== ''
                ? '選択箇所へのコメント'
                : '全体コメントを追加'
            }
            className="flex-1 border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] px-2.5 py-1.5 font-mono text-[12px] text-[color:var(--color-text-primary)] outline-none focus:border-[color:var(--color-text-tertiary)]"
          />
          <button
            type="button"
            onClick={submit}
            disabled={draft.trim() === '' || create.isPending}
            className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-3 py-1.5 font-mono text-[12px] text-[color:var(--color-text-primary)] hover:border-[color:var(--color-accent-strategy)] hover:text-[color:var(--color-accent-strategy)] disabled:opacity-50"
          >
            送信
          </button>
        </div>
        {create.isError && (
          <p className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]">
            送信に失敗しました
          </p>
        )}
      </div>
    </section>
  )
}

function CommentRow({
  comment,
  replies,
}: {
  comment: Comment
  replies: Comment[]
}) {
  return (
    <div className="px-3.5 py-3 text-[13px]">
      <CommentBody c={comment} />
      {replies.map((r) => (
        <div
          key={r.id}
          className="mt-2.5 border-l border-[color:var(--color-hairline)] pl-3"
        >
          <CommentBody c={r} />
        </div>
      ))}
    </div>
  )
}

function CommentBody({ c }: { c: Comment }) {
  const isLLM = c.author_kind === 'llm'
  // 「> 引用」プレフィクスを抽出して quote box に分離する。残りはコメント本文。
  const quote = extractQuote(c.body)
  return (
    <div className="space-y-1.5">
      <div className="flex items-center gap-2 font-mono text-[11px]">
        <span
          className={`inline-flex size-4 items-center justify-center border text-[10px] font-bold ${
            isLLM
              ? 'border-[color:var(--color-accent-strategy)] text-[color:var(--color-accent-strategy)]'
              : 'border-[color:var(--color-text-tertiary)] text-[color:var(--color-text-primary)]'
          }`}
        >
          {isLLM ? '>' : c.author_label.slice(0, 1).toUpperCase()}
        </span>
        <span className="font-bold text-[color:var(--color-text-primary)]">
          {c.author_label}
        </span>
        {isLLM && (
          <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
            LLM
          </span>
        )}
        <span className="ml-auto text-[color:var(--color-text-tertiary)]">
          {formatRelative(c.created_at)}
        </span>
      </div>
      {quote != null && (
        <div className="whitespace-pre-wrap border-l-2 border-[color:var(--color-border-strategy)] bg-[color:var(--panel-inset)] px-2 py-1 text-[12px] italic text-[color:var(--color-text-tertiary)]">
          “{quote.text}”
        </div>
      )}
      <div className="whitespace-pre-wrap leading-relaxed text-[color:var(--color-text-primary)]">
        {quote?.body ?? c.body}
      </div>
    </div>
  )
}

function extractQuote(body: string): { text: string; body: string } | null {
  const lines = body.split('\n')
  if (lines[0] == null || !lines[0].startsWith('> ')) return null
  let i = 0
  const quoteLines: string[] = []
  for (; i < lines.length; i += 1) {
    const line = lines[i]
    if (line == null || !line.startsWith('> ')) break
    quoteLines.push(line.slice(2))
  }
  while (i < lines.length && (lines[i] ?? '').trim() === '') i += 1
  return {
    text: quoteLines.join('\n'),
    body: lines.slice(i).join('\n'),
  }
}
