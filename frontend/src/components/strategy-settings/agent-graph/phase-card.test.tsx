import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PhaseCard } from '#components/strategy-settings/agent-graph/phase-card'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

afterEach(cleanup)

const PLAN: AgentGraphPhaseForm = {
  key: 'plan',
  label: '見出し案',
  model: 'fictional-model-orbit',
  prompt: 'テーマを読みやすい節に分ける',
  skills: [],
  tools: [],
  output: {},
}

const COMPOSE: AgentGraphPhaseForm = {
  ...PLAN,
  key: 'compose',
  label: '節の作成',
  forEach: 'plan.sections',
}

const NOOP_HANDLERS = {
  onLabelChange: () => {},
  onMoveUp: () => {},
  onMoveDown: () => {},
  onRemove: () => {},
}

describe('PhaseCard', () => {
  it('番号・label・モデルを表示する', () => {
    render(
      <PhaseCard index={0} total={2} phase={PLAN} {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByText('1')).toBeInTheDocument()
    expect(screen.getByLabelText('フェーズ名')).toHaveValue('見出し案')
    expect(screen.getByText('fictional-model-orbit')).toBeInTheDocument()
  })

  it('フェーズ名を編集すると onLabelChange が呼ばれる', async () => {
    const user = userEvent.setup()
    const onLabelChange = vi.fn()
    render(
      <PhaseCard
        index={0}
        total={1}
        phase={PLAN}
        {...NOOP_HANDLERS}
        onLabelChange={onLabelChange}
      >
        <span>field</span>
      </PhaseCard>,
    )
    await user.type(screen.getByLabelText('フェーズ名'), 'X')
    expect(onLabelChange).toHaveBeenCalledWith('見出し案X')
  })

  it('for_each を持つフェーズは参照先ラベルと対象フィールドを含む説明文を表示する', () => {
    render(
      <PhaseCard
        index={1}
        total={2}
        phase={COMPOSE}
        referencedLabel="見出し案"
        {...NOOP_HANDLERS}
      >
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByText(/見出し案 が返す/)).toBeInTheDocument()
    expect(screen.getByText('sections[]')).toBeInTheDocument()
  })

  it('for_each を持たないフェーズには説明文を出さない', () => {
    render(
      <PhaseCard index={0} total={1} phase={PLAN} {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.queryByText(/要素ごとに実行されます/)).toBeNull()
  })

  it('hasError のとき phase-error を表示する', () => {
    render(
      <PhaseCard index={0} total={1} phase={PLAN} hasError {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByTestId('phase-error')).toBeInTheDocument()
  })

  it('先頭では上移動、末尾では下移動が disabled になる', () => {
    render(
      <PhaseCard index={0} total={2} phase={PLAN} {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(
      screen.getByRole('button', { name: 'フェーズ 見出し案 を上に移動' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'フェーズ 見出し案 を下に移動' }),
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
        {...NOOP_HANDLERS}
        onMoveUp={onMoveUp}
        onMoveDown={onMoveDown}
        onRemove={onRemove}
      >
        <span>field</span>
      </PhaseCard>,
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 見出し案 を上に移動' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 見出し案 を下に移動' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 見出し案 を削除' }),
    )
    expect(onMoveUp).toHaveBeenCalledTimes(1)
    expect(onMoveDown).toHaveBeenCalledTimes(1)
    expect(onRemove).toHaveBeenCalledTimes(1)
  })

  it('children (フィールド) を描画する', () => {
    render(
      <PhaseCard index={0} total={1} phase={PLAN} {...NOOP_HANDLERS}>
        <span>custom field</span>
      </PhaseCard>,
    )
    expect(screen.getByText('custom field')).toBeInTheDocument()
  })
})
