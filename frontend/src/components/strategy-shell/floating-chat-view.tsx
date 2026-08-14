import { Link } from '@tanstack/react-router'
import { X } from 'lucide-react'

import {
  type AgentGraphPhaseSummary,
  TaskExecutionTree,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'
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
  seed: string | null
  input: string
  status: FloatingChatStatus
  notes: FloatingChatNote[]
  steps: TaskStep[]
  configPhases: AgentGraphPhaseSummary[]
  traceUrlTemplate?: string
  onOpen: () => void
  onClose: () => void
  onInputChange: (value: string) => void
  onSubmit: () => void
}

export function FloatingChatView({
  open,
  strategyId,
  seed,
  input,
  status,
  notes,
  steps,
  configPhases,
  traceUrlTemplate,
  onOpen,
  onClose,
  onInputChange,
  onSubmit,
}: FloatingChatViewProps): React.ReactElement {
  if (!open) {
    return (
      <button
        type="button"
        onClick={onOpen}
        title="アナリストを呼ぶ (on-demand)"
        aria-label="アナリストを呼ぶ"
        className="fixed bottom-5 right-5 z-[60] grid h-12 w-12 cursor-pointer place-items-center border border-[color:var(--color-accent-strategy)] bg-[color:var(--color-bg-secondary)] font-mono text-[22px] font-bold text-[color:var(--color-accent-strategy)] hover:bg-[color:var(--color-accent-strategy)] hover:text-white"
      >
        &gt;_
      </button>
    )
  }

  const submitting = status.kind === 'submitting'
  const polling = status.kind === 'polling'
  const inputDisabled = strategyId == null || submitting || polling
  const submitDisabled = inputDisabled || input.trim() === ''
  const placeholder =
    strategyId == null ? '戦略ホームを開いてください' : 'プロンプトを入力'

  return (
    <div
      role="dialog"
      aria-label="on-demand session"
      className="fixed bottom-5 right-5 z-[60] flex h-[580px] max-h-[calc(100vh-100px)] w-[420px] max-w-[calc(100vw-28px)] flex-col border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-secondary)]"
    >
      <div className="flex items-center gap-2.5 border-b border-[color:var(--color-border-strategy)] px-3.5 py-2.5">
        <span className="flex items-baseline gap-1.5 font-mono text-[13px] font-bold">
          <span className="text-[color:var(--color-accent-strategy)]">
            &gt;_
          </span>
          <span>on-demand session</span>
        </span>
        <button
          type="button"
          onClick={onClose}
          aria-label="閉じる"
          className="ml-auto cursor-pointer text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)]"
        >
          <X className="size-4" />
        </button>
      </div>
      <div className="flex-1 space-y-3 overflow-y-auto p-3.5 text-[13px] text-[color:var(--color-text-secondary)]">
        {seed != null && (
          <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] p-3">
            <div className="mb-2 font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
              seed
            </div>
            <p className="leading-relaxed text-[color:var(--color-text-primary)]">
              {seed}
            </p>
          </div>
        )}
        <FloatingChatStatusBlock
          status={status}
          notes={notes}
          strategyId={strategyId}
          steps={steps}
          configPhases={configPhases}
          traceUrlTemplate={traceUrlTemplate}
        />
      </div>
      <form
        onSubmit={(e) => {
          e.preventDefault()
          if (submitDisabled) return
          onSubmit()
        }}
        className="flex items-center gap-2 border-t border-[color:var(--color-border-strategy)] px-3.5 py-3"
      >
        <span className="font-mono font-bold text-[color:var(--color-accent-strategy)]">
          &gt;
        </span>
        <input
          aria-label="メッセージ入力"
          value={input}
          onChange={(e) => {
            onInputChange(e.target.value)
          }}
          disabled={inputDisabled}
          placeholder={placeholder}
          className="flex-1 border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] px-2.5 py-2 font-mono text-[13px] text-[color:var(--color-text-primary)] outline-none disabled:opacity-60"
        />
        <button
          type="submit"
          disabled={submitDisabled}
          aria-label="送信"
          className="border border-[color:var(--color-accent-strategy)] bg-[color:var(--color-bg-primary)] px-3 py-2 font-mono text-[12px] font-bold text-[color:var(--color-accent-strategy)] hover:bg-[color:var(--color-accent-strategy)] hover:text-white disabled:cursor-not-allowed disabled:opacity-50 disabled:hover:bg-[color:var(--color-bg-primary)] disabled:hover:text-[color:var(--color-accent-strategy)]"
        >
          send
        </button>
      </form>
    </div>
  )
}

function FloatingChatStatusBlock({
  status,
  notes,
  strategyId,
  steps,
  configPhases,
  traceUrlTemplate,
}: {
  status: FloatingChatStatus
  notes: FloatingChatNote[]
  strategyId: string | null
  steps: TaskStep[]
  configPhases: AgentGraphPhaseSummary[]
  traceUrlTemplate?: string
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
        <div className="space-y-2">
          <StatusLine label={status.phase} message="アナリストが分析中です…" />
          <TaskExecutionTree
            steps={steps}
            configPhases={configPhases}
            traceUrlTemplate={traceUrlTemplate}
          />
        </div>
      )
    case 'completed':
      return (
        <div className="space-y-2">
          <StatusLine label="completed" message="分析が完了しました。" />
          <TaskExecutionTree
            steps={steps}
            configPhases={configPhases}
            traceUrlTemplate={traceUrlTemplate}
          />
          <FloatingChatNoteList notes={notes} strategyId={strategyId} />
        </div>
      )
    case 'failed':
      return (
        <div className="space-y-2">
          <StatusLine label="failed" message="タスクが失敗しました。" />
          <TaskExecutionTree
            steps={steps}
            configPhases={configPhases}
            traceUrlTemplate={traceUrlTemplate}
          />
          {status.error_summary != null && status.error_summary !== '' && (
            <pre className="whitespace-pre-wrap border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] p-2 font-mono text-[11px] text-[color:var(--color-text-secondary)]">
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
    <p className="flex items-baseline gap-2 font-mono text-[12px]">
      <span className="uppercase tracking-wider text-[color:var(--color-accent-strategy)]">
        {label}
      </span>
      <span className="text-[color:var(--color-text-secondary)]">
        {message}
      </span>
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
      <p className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
        生成ノートはまだ取得できていません。
      </p>
    )
  }
  return (
    <ul className="space-y-1">
      {notes.map((n) => (
        <li key={n.id}>
          <Link
            to="/strategies/$id/notes/$noteId"
            params={{ id: strategyId, noteId: n.id }}
            className="flex items-baseline gap-2 border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] px-2.5 py-1.5 hover:border-[color:var(--color-accent-strategy)]"
          >
            <span className="flex-1 text-[12px] text-[color:var(--color-text-primary)]">
              {n.title}
            </span>
            <span className="font-mono text-[10px] text-[color:var(--color-text-tertiary)]">
              {formatRelative(n.updated_at)}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  )
}
