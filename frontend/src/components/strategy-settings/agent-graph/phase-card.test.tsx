import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PhaseCard } from '#components/strategy-settings/agent-graph/phase-card'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

afterEach(cleanup)

const PLAN: AgentGraphPhaseForm = {
  key: 'plan',
  label: '調査計画',
  model: 'claude-opus-4',
  prompt: '仮説を立てよ',
  skills: [],
  tools: [],
  output: {},
}

const INVESTIGATE: AgentGraphPhaseForm = {
  ...PLAN,
  key: 'investigate',
  label: '仮説の調査',
  forEach: 'plan.hypotheses',
}

describe('PhaseCard', () => {
  it('番号・label・モデルを表示する', () => {
    render(
      <PhaseCard
        index={0}
        total={2}
        phase={PLAN}
        onLabelChange={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onRemove={() => {}}
      >
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByText('1')).toBeInTheDocument()
    expect(screen.getByLabelText('フェーズ名')).toHaveValue('調査計画')
    expect(screen.getByText('claude-opus-4')).toBeInTheDocument()
  })

  it('フェーズ名を編集すると onLabelChange が呼ばれる', async () => {
    const user = userEvent.setup()
    const onLabelChange = vi.fn()
    render(
      <PhaseCard
        index={0}
        total={1}
        phase={PLAN}
        onLabelChange={onLabelChange}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onRemove={() => {}}
      >
        <span>field</span>
      </PhaseCard>,
    )
    await user.type(screen.getByLabelText('フェーズ名'), 'X')
    expect(onLabelChange).toHaveBeenCalledWith('調査計画X')
  })

  it('for_each を持つフェーズは参照先ラベルと対象フィールドを含む説明文を表示する', () => {
    render(
      <PhaseCard
        index={1}
        total={2}
        phase={INVESTIGATE}
        referencedLabel="調査計画"
        onLabelChange={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onRemove={() => {}}
      >
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByText(/調査計画 が返す/)).toBeInTheDocument()
    expect(screen.getByText('hypotheses[]')).toBeInTheDocument()
  })

  it('for_each を持たないフェーズには説明文を出さない', () => {
    render(
      <PhaseCard
        index={0}
        total={1}
        phase={PLAN}
        onLabelChange={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onRemove={() => {}}
      >
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.queryByText(/要素ごとに実行されます/)).toBeNull()
  })

  it('hasError のとき phase-error を表示する', () => {
    render(
      <PhaseCard
        index={0}
        total={1}
        phase={PLAN}
        hasError
        onLabelChange={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onRemove={() => {}}
      >
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByTestId('phase-error')).toBeInTheDocument()
  })

  it('先頭では上移動、末尾では下移動が disabled になる', () => {
    render(
      <PhaseCard
        index={0}
        total={2}
        phase={PLAN}
        onLabelChange={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onRemove={() => {}}
      >
        <span>field</span>
      </PhaseCard>,
    )
    expect(
      screen.getByRole('button', { name: 'フェーズ 調査計画 を上に移動' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'フェーズ 調査計画 を下に移動' }),
    ).not.toBeDisabled()
  })

  it('上下移動・削除ボタンがハンドラを呼ぶ', async () => {
    const user = userEvent.setup()
    const onMoveUp = vi.fn()
    const onMoveDown = vi.fn()
    const onRemove = vi.fn()
    render(
      <PhaseCard
        index={1}
        total={3}
        phase={PLAN}
        onLabelChange={() => {}}
        onMoveUp={onMoveUp}
        onMoveDown={onMoveDown}
        onRemove={onRemove}
      >
        <span>field</span>
      </PhaseCard>,
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 調査計画 を上に移動' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 調査計画 を下に移動' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 調査計画 を削除' }),
    )
    expect(onMoveUp).toHaveBeenCalledTimes(1)
    expect(onMoveDown).toHaveBeenCalledTimes(1)
    expect(onRemove).toHaveBeenCalledTimes(1)
  })

  it('children (フィールド) を描画する', () => {
    render(
      <PhaseCard
        index={0}
        total={1}
        phase={PLAN}
        onLabelChange={() => {}}
        onMoveUp={() => {}}
        onMoveDown={() => {}}
        onRemove={() => {}}
      >
        <span>custom field</span>
      </PhaseCard>,
    )
    expect(screen.getByText('custom field')).toBeInTheDocument()
  })
})
