import type { ReactNode } from 'react'

import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'
import { Input } from '#components/ui/input'
import { cn } from '#lib/utils'

interface PhaseCardProps {
  index: number
  total: number
  phase: AgentGraphPhaseForm
  /** for_each が参照するフェーズの label (説明文の表示用) */
  referencedLabel?: string
  hasError?: boolean
  onLabelChange: (next: string) => void
  onMoveUp: () => void
  onMoveDown: () => void
  onRemove: () => void
  children: ReactNode
}

export function PhaseCard({
  index,
  total,
  phase,
  referencedLabel,
  hasError = false,
  onLabelChange,
  onMoveUp,
  onMoveDown,
  onRemove,
  children,
}: PhaseCardProps) {
  const forEachField = phase.forEach?.split('.')[1]
  const hasValidForEach = phase.forEach != null && forEachField != null

  const card = (
    <div
      data-testid={`phase-card-${phase.key}`}
      className={cn(
        'border bg-bg-secondary p-3',
        hasError ? 'border-primary' : 'border-border',
      )}
    >
      <div className="flex items-center gap-2.5">
        <span className="flex h-5 w-5 items-center justify-center bg-bg-tertiary font-mono text-2xs font-bold text-muted-foreground-strong">
          {index + 1}
        </span>
        <Input
          aria-label="フェーズ名"
          value={phase.label}
          onChange={(e) => {
            onLabelChange(e.target.value)
          }}
          className="h-auto w-40 rounded-none border-transparent bg-transparent px-1 py-0.5 font-bold text-sm text-foreground hover:border-border focus-visible:border-border focus-visible:bg-background focus-visible:ring-0"
        />
        <span className="border border-border bg-bg-tertiary px-1.5 py-0.5 font-mono text-2xs text-muted-foreground-strong">
          {phase.model === '' ? '(未設定)' : phase.model}
        </span>
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            aria-label={`フェーズ ${phase.label} を上に移動`}
            disabled={index === 0}
            onClick={onMoveUp}
            className="px-1 text-muted-foreground hover:text-foreground disabled:opacity-30"
          >
            ↑
          </button>
          <button
            type="button"
            aria-label={`フェーズ ${phase.label} を下に移動`}
            disabled={index === total - 1}
            onClick={onMoveDown}
            className="px-1 text-muted-foreground hover:text-foreground disabled:opacity-30"
          >
            ↓
          </button>
          <button
            type="button"
            aria-label={`フェーズ ${phase.label} を削除`}
            onClick={onRemove}
            className="px-1 font-mono text-2xs text-muted-foreground hover:text-primary"
          >
            削除
          </button>
        </div>
      </div>

      {hasValidForEach && (
        <div className="mt-1 font-mono text-2xs text-muted-foreground">
          {referencedLabel ?? phase.forEach?.split('.')[0]} が返す{' '}
          <span className="text-muted-foreground-strong">{forEachField}[]</span>{' '}
          の要素ごとに実行されます
        </div>
      )}

      {hasError && (
        <div
          data-testid="phase-error"
          className="mt-1 font-mono text-2xs text-primary"
        >
          このフェーズの設定を確認してください
        </div>
      )}

      <div className="mt-2.5 grid grid-cols-(--grid-cols-field-label) items-start gap-x-3 gap-y-2 text-xs">
        {children}
      </div>
    </div>
  )

  if (phase.forEach == null) return card

  return (
    <div className="grid grid-cols-(--grid-cols-foreach-indent)">
      <div
        aria-hidden
        className="mb-6 ml-2.5 border-b border-l border-border"
      />
      <div className="ml-2.5">{card}</div>
    </div>
  )
}
