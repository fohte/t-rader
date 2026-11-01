import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PhaseCard } from '#components/strategy-settings/agent-graph/phase-card'
import type { AgentGraphPhaseForm } from '#components/strategy-settings/agent-graph/types'

afterEach(cleanup)

const ALPHA: AgentGraphPhaseForm = {
  key: 'alpha',
  label: 'サンプル',
  model: 'fictional-model-orbit',
  prompt: 'sample prompt',
  skills: [],
  tools: [],
  output: {},
}

const BETA: AgentGraphPhaseForm = {
  ...ALPHA,
  key: 'beta',
  label: '次の段階',
  forEach: 'alpha.records',
}

const NOOP_HANDLERS = {
  onLabelChange: () => {},
  onKindChange: () => {},
  onMoveUp: () => {},
  onMoveDown: () => {},
  onRemove: () => {},
}

describe('PhaseCard', () => {
  it('番号・label・モデルを表示する', () => {
    render(
      <PhaseCard index={0} total={2} phase={ALPHA} {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByText('1')).toBeInTheDocument()
    expect(screen.getByLabelText('フェーズ名')).toHaveValue('サンプル')
    expect(screen.getByText('fictional-model-orbit')).toBeInTheDocument()
  })

  it('フェーズ名を編集すると onLabelChange が呼ばれる', async () => {
    const user = userEvent.setup()
    const onLabelChange = vi.fn()
    render(
      <PhaseCard
        index={0}
        total={1}
        phase={ALPHA}
        {...NOOP_HANDLERS}
        onLabelChange={onLabelChange}
      >
        <span>field</span>
      </PhaseCard>,
    )
    await user.type(screen.getByLabelText('フェーズ名'), 'X')
    expect(onLabelChange).toHaveBeenCalledWith('サンプルX')
  })

  it('for_each を持つフェーズは参照先ラベルと対象フィールドを含む説明文を表示する', () => {
    render(
      <PhaseCard
        index={1}
        total={2}
        phase={BETA}
        referencedLabel="サンプル"
        {...NOOP_HANDLERS}
      >
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByText(/サンプル が返す/)).toBeInTheDocument()
    expect(screen.getByText('records[]')).toBeInTheDocument()
  })

  it('for_each を持たないフェーズには説明文を出さない', () => {
    render(
      <PhaseCard index={0} total={1} phase={ALPHA} {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.queryByText(/要素ごとに実行されます/)).toBeNull()
  })

  it('hasError のとき phase-error を表示する', () => {
    render(
      <PhaseCard index={0} total={1} phase={ALPHA} hasError {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(screen.getByTestId('phase-error')).toBeInTheDocument()
  })

  it('先頭では上移動、末尾では下移動が disabled になる', () => {
    render(
      <PhaseCard index={0} total={2} phase={ALPHA} {...NOOP_HANDLERS}>
        <span>field</span>
      </PhaseCard>,
    )
    expect(
      screen.getByRole('button', { name: 'フェーズ サンプル を上に移動' }),
    ).toBeDisabled()
    expect(
      screen.getByRole('button', { name: 'フェーズ サンプル を下に移動' }),
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
        phase={ALPHA}
        {...NOOP_HANDLERS}
        onMoveUp={onMoveUp}
        onMoveDown={onMoveDown}
        onRemove={onRemove}
      >
        <span>field</span>
      </PhaseCard>,
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ サンプル を上に移動' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ サンプル を下に移動' }),
    )
    await user.click(
      screen.getByRole('button', { name: 'フェーズ サンプル を削除' }),
    )
    expect(onMoveUp).toHaveBeenCalledTimes(1)
    expect(onMoveDown).toHaveBeenCalledTimes(1)
    expect(onRemove).toHaveBeenCalledTimes(1)
  })

  it('children (フィールド) を描画する', () => {
    render(
      <PhaseCard index={0} total={1} phase={ALPHA} {...NOOP_HANDLERS}>
        <span>custom field</span>
      </PhaseCard>,
    )
    expect(screen.getByText('custom field')).toBeInTheDocument()
  })
})
