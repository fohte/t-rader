import { Link } from '@tanstack/react-router'
import { X } from 'lucide-react'

import { MarkdownBody } from '#components/note-detail/markdown-body'
import { PurposeSelect } from '#components/purpose-select'
import { formatRelative } from '#lib/note-utils'

export interface FloatingChatNote {
  id: string
  title: string
  updated_at: string
}

export type FloatingChatStatus =
  | { kind: 'idle' }
  | { kind: 'submitting' }
  | { kind: 'polling'; phase: 'pending' | 'running' }
  | { kind: 'completed' }
  | { kind: 'failed'; error_summary: string | null }
  | { kind: 'error'; message: string }

export interface FloatingChatViewProps {
  open: boolean
  strategyId: string | null
  taskStrategyId: string | null
  strategies: { id: string; name: string }[]
  submittedPrompt: string | null
  seed: string | null
  input: string
  resultText: string | null
  status: FloatingChatStatus
  notes: FloatingChatNote[]
  purposes: string[]
  selectedPurpose: string
  /** 直近に投入したタスクの ID。詳細画面への導線を出すために使う (未投入なら null) */
  currentTaskId: string | null
  onOpen: () => void
  onClose: () => void
  onInputChange: (value: string) => void
  onStrategyChange: (value: string | null) => void
  onPurposeChange: (value: string) => void
  onSubmit: () => void
}

export function FloatingChatView({
  open,
  strategyId,
  taskStrategyId,
  strategies,
  submittedPrompt,
  seed,
  input,
  resultText,
  status,
  notes,
  purposes,
  selectedPurpose,
  currentTaskId,
  onOpen,
  onClose,
  onInputChange,
  onStrategyChange,
  onPurposeChange,
  onSubmit,
}: FloatingChatViewProps): React.ReactElement {
  if (!open) {
    return (
      <button
        type="button"
        onClick={onOpen}
        title="アナリストを呼ぶ (on-demand)"
        aria-label="アナリストを呼ぶ"
        className="fixed bottom-5 right-5 z-floating-chat grid h-12 w-12 cursor-pointer place-items-center border border-primary bg-bg-secondary font-mono text-2xl font-bold text-primary hover:bg-primary hover:text-white"
      >
        &gt;_
      </button>
    )
  }

  const submitting = status.kind === 'submitting'
  const polling = status.kind === 'polling'
  const inputDisabled = submitting || polling
  const submitDisabled =
    strategyId == null || inputDisabled || input.trim() === ''

  return (
    <div
      role="dialog"
      aria-label="on-demand session"
      className="fixed inset-0 z-floating-chat flex h-dvh max-h-none w-screen max-w-none flex-col border border-border bg-bg-secondary md:inset-auto md:bottom-5 md:right-5 md:h-145 md:max-h-floating-chat-h md:w-105 md:max-w-floating-chat-w"
    >
      <div className="flex items-center gap-2.5 border-b border-border px-3.5 py-2.5">
        <span className="flex items-baseline gap-1.5 font-mono text-sm font-bold">
          <span className="text-primary">&gt;_</span>
          <span>on-demand session</span>
        </span>
        <select
          aria-label="戦略"
          value={strategyId ?? ''}
          disabled={submitting || polling}
          onChange={(event) => {
            onStrategyChange(
              event.target.value === '' ? null : event.target.value,
            )
          }}
          className="min-w-0 flex-1 border border-border bg-background px-2 py-1 font-mono text-xs text-foreground disabled:opacity-60"
        >
          <option value="">戦略を選択</option>
          {strategyId != null &&
            !strategies.some((strategy) => strategy.id === strategyId) && (
              <option value={strategyId}>現在の戦略</option>
            )}
          {strategies.map((strategy) => (
            <option key={strategy.id} value={strategy.id}>
              {strategy.name}
            </option>
          ))}
        </select>
        <button
          type="button"
          onClick={onClose}
          aria-label="閉じる"
          className="ml-auto cursor-pointer text-muted-foreground hover:text-foreground"
        >
          <X className="size-4" />
        </button>
      </div>
      <div className="flex-1 space-y-3 overflow-y-auto p-3.5 text-sm text-muted-foreground-strong">
        {seed != null && (
          <div className="border border-border bg-background p-3">
            <div className="mb-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
              seed
            </div>
            <p className="leading-relaxed text-foreground">{seed}</p>
          </div>
        )}
        {submittedPrompt != null && (
          <div
            data-message-role="user"
            className="ml-auto max-w-80 border border-border bg-background p-3 leading-relaxed text-foreground"
          >
            {submittedPrompt}
          </div>
        )}
        <FloatingChatStatusBlock
          status={status}
          notes={notes}
          strategyId={taskStrategyId}
          resultText={resultText}
        />
        {taskStrategyId != null && currentTaskId != null && (
          <Link
            to="/strategies/$id/runs/$taskId"
            params={{ id: taskStrategyId, taskId: currentTaskId }}
            className="block font-mono text-2xs text-primary hover:underline"
          >
            → 実行の詳細を見る
          </Link>
        )}
      </div>
      <form
        onSubmit={(e) => {
          e.preventDefault()
          if (submitDisabled) return
          onSubmit()
        }}
        className="space-y-2 border-t border-border px-3.5 py-3"
      >
        <PurposeSelect
          purposes={purposes}
          selectedPurpose={selectedPurpose}
          onPurposeChange={onPurposeChange}
          disabled={inputDisabled}
        />
        <div className="flex items-center gap-2">
          <span className="font-mono font-bold text-primary">&gt;</span>
          <input
            aria-label="メッセージ入力"
            value={input}
            onChange={(e) => {
              onInputChange(e.target.value)
            }}
            disabled={inputDisabled}
            placeholder="プロンプトを入力"
            className="flex-1 border border-border bg-background px-2.5 py-2 font-mono text-sm text-foreground outline-none disabled:opacity-60"
          />
          <button
            type="submit"
            disabled={submitDisabled}
            aria-label="送信"
            className="border border-primary bg-background px-3 py-2 font-mono text-xs font-bold text-primary hover:bg-primary hover:text-white disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-background disabled:hover:text-primary"
          >
            send
          </button>
        </div>
      </form>
    </div>
  )
}

function FloatingChatStatusBlock({
  status,
  notes,
  strategyId,
  resultText,
}: {
  status: FloatingChatStatus
  notes: FloatingChatNote[]
  strategyId: string | null
  resultText: string | null
}): React.ReactElement {
  switch (status.kind) {
    case 'idle':
      return (
        <p className="leading-relaxed">
          プロンプトを送るとアナリストが起動します。
        </p>
      )
    case 'submitting':
      return <StatusLine label="submitting" message="タスクを投入しています…" />
    case 'polling':
      return (
        <StatusLine label={status.phase} message="アナリストが分析中です…" />
      )
    case 'completed':
      return (
        <div className="space-y-2">
          <StatusLine label="completed" message="分析が完了しました。" />
          {resultText == null || resultText === '' ? (
            <p
              data-message-role="assistant"
              className="border-l-2 border-primary pl-3 text-sm text-muted-foreground-strong"
            >
              返信本文はありません。
            </p>
          ) : (
            <div
              aria-label="エージェントの返信"
              data-message-role="assistant"
              className="min-w-0 border-l-2 border-primary pl-3 text-sm"
            >
              <MarkdownBody source={resultText} />
            </div>
          )}
          <FloatingChatNoteList notes={notes} strategyId={strategyId} />
        </div>
      )
    case 'failed':
      return (
        <div className="space-y-2">
          <StatusLine label="failed" message="タスクが失敗しました。" />
          {status.error_summary != null && status.error_summary !== '' && (
            <pre className="whitespace-pre-wrap border border-border bg-background p-2 font-mono text-2xs text-muted-foreground-strong">
              {status.error_summary}
            </pre>
          )}
        </div>
      )
    case 'error':
      return <StatusLine label="error" message={status.message} />
  }
}

function StatusLine({
  label,
  message,
}: {
  label: string
  message: string
}): React.ReactElement {
  return (
    <p className="flex items-baseline gap-2 font-mono text-xs">
      <span className="uppercase tracking-wider text-primary">{label}</span>
      <span className="text-muted-foreground-strong">{message}</span>
    </p>
  )
}

function FloatingChatNoteList({
  notes,
  strategyId,
}: {
  notes: FloatingChatNote[]
  strategyId: string | null
}): React.ReactElement {
  if (strategyId == null || notes.length === 0) {
    return (
      <p className="font-mono text-2xs text-muted-foreground">
        生成ノートはまだ取得できていません。
      </p>
    )
  }
  return (
    <ul className="space-y-1">
      {notes.map((n) => (
        <li key={n.id}>
          <Link
            to="/notes/$noteId"
            params={{ noteId: n.id }}
            className="flex items-baseline gap-2 border border-border bg-background px-2.5 py-1.5 hover:border-primary"
          >
            <span className="flex-1 text-xs text-foreground">{n.title}</span>
            <span className="font-mono text-2xs text-muted-foreground">
              {formatRelative(n.updated_at)}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  )
}
