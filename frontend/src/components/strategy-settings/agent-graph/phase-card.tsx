import type { ReactNode } from 'react'

import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'
import { cn } from '#lib/utils'

interface PhaseCardProps {
  index: number
  total: number
  phase: AgentGraphPhaseForm
  /** for_each が参照するフェーズの label (説明文の表示用) */
  referencedLabel?: string
  hasError?: boolean
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
  onMoveUp,
  onMoveDown,
  onRemove,
  children,
}: PhaseCardProps) {
  const forEachField = phase.forEach?.split('.')[1]

  const card = (
    <div
      data-testid={`phase-card-${phase.key}`}
      className={cn(
        'border bg-[color:var(--color-bg-secondary)] p-3',
        hasError
          ? 'border-[color:var(--color-accent-strategy)]'
          : 'border-[color:var(--color-border-strategy)]',
      )}
    >
      <div className="flex items-center gap-2.5">
        <span className="flex h-[21px] w-[21px] items-center justify-center bg-[color:var(--color-bg-tertiary)] font-mono text-[11px] font-bold text-[color:var(--color-text-secondary)]">
          {index + 1}
        </span>
        <span className="text-[13.5px] font-bold text-[color:var(--color-text-primary)]">
          {phase.label}
        </span>
        <span className="border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-tertiary)] px-1.5 py-0.5 font-mono text-[10px] text-[color:var(--color-text-secondary)]">
          {phase.model === '' ? '(未設定)' : phase.model}
        </span>
        <div className="ml-auto flex items-center gap-1">
          <button
            type="button"
            aria-label={`フェーズ ${phase.label} を上に移動`}
            disabled={index === 0}
            onClick={onMoveUp}
            className="px-1 text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)] disabled:opacity-30"
          >
            ↑
          </button>
          <button
            type="button"
            aria-label={`フェーズ ${phase.label} を下に移動`}
            disabled={index === total - 1}
            onClick={onMoveDown}
            className="px-1 text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-text-primary)] disabled:opacity-30"
          >
            ↓
          </button>
          <button
            type="button"
            aria-label={`フェーズ ${phase.label} を削除`}
            onClick={onRemove}
            className="px-1 font-mono text-[11px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-accent-strategy)]"
          >
            削除
          </button>
        </div>
      </div>

      {phase.forEach != null && (
        <div className="mt-1 font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
          {referencedLabel ?? phase.forEach.split('.')[0]} が返す{' '}
          <span className="text-[color:var(--color-text-secondary)]">
            {forEachField}[]
          </span>{' '}
          の要素ごとに実行されます
        </div>
      )}

      {hasError && (
        <div
          data-testid="phase-error"
          className="mt-1 font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
        >
          このフェーズの設定を確認してください
        </div>
      )}

      <div className="mt-2.5 grid grid-cols-[108px_1fr] items-start gap-x-3 gap-y-2 text-[12px]">
        {children}
      </div>
    </div>
  )

  if (phase.forEach == null) return card

  return (
    <div className="grid grid-cols-[22px_1fr]">
      <div
        aria-hidden
        className="mb-6 ml-[10px] border-b border-l border-[color:var(--color-border-strategy)]"
      />
      <div className="ml-2.5">{card}</div>
    </div>
  )
}
