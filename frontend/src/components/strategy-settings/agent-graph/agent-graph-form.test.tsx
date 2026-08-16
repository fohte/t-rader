import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useEffect, useRef, useState } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AgentGraphForm } from '#components/strategy-settings/agent-graph/agent-graph-form'
import { parseAgentGraphPhases } from '#components/strategy-settings/agent-graph/document'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)

afterEach(cleanup)

const SAMPLE = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    prompt: 仮説を立てよ
  - key: investigate
    label: 仮説の調査
    model: deepseek-v4-flash
    for_each: plan.hypotheses
    prompt: 割り当てられた仮説を検証せよ
`

// value を state で持ち、onChange で更新する controlled wrapper。実際の agent-graph-editor.tsx
// も同様の構造で AgentGraphForm を使うため、実運用に近い形でテストする。
function Controlled({
  initial,
  errorPhaseKey,
}: {
  initial: string
  errorPhaseKey?: string | null
}) {
  const [value, setValue] = useState(initial)
  const lastEnabledValueRef = useRef(value)
  useEffect(() => {
    if (value.trim() !== '') lastEnabledValueRef.current = value
  }, [value])
  return (
    <>
      <AgentGraphForm
        value={value}
        onChange={setValue}
        errorPhaseKey={errorPhaseKey}
        lastEnabledValueRef={lastEnabledValueRef}
      />
      {/* onChange 経由で value (YAML) に実際に反映されたかをテストから検証するための出力 */}
      <pre data-testid="yaml-value">{value}</pre>
    </>
  )
}

describe('AgentGraphForm', () => {
  it('value が空文字列なら分割トグルが off で表示され、カードは出ない', () => {
    render(<Controlled initial="" />)
    expect(screen.getByLabelText('フェーズ分割を有効にする')).not.toBeChecked()
    expect(screen.queryByTestId('phase-card-plan')).toBeNull()
  })

  it('value がフェーズを含めば分割トグルが on で、フェーズカードを表示する', () => {
    render(<Controlled initial={SAMPLE} />)
    expect(screen.getByLabelText('フェーズ分割を有効にする')).toBeChecked()
    expect(screen.getByTestId('phase-card-plan')).toBeInTheDocument()
    expect(screen.getByTestId('phase-card-investigate')).toBeInTheDocument()
  })

  it('トグルを off にすると値が空文字列になり、on に戻すと復元される', async () => {
    const user = userEvent.setup()
    render(<Controlled initial={SAMPLE} />)

    await user.click(screen.getByLabelText('フェーズ分割を有効にする'))
    expect(screen.queryByTestId('phase-card-plan')).toBeNull()

    await user.click(screen.getByLabelText('フェーズ分割を有効にする'))
    expect(screen.getByTestId('phase-card-plan')).toBeInTheDocument()
    expect(screen.getByTestId('phase-card-investigate')).toBeInTheDocument()
  })

  it('未設定の状態でトグルを on にすると最小構成のフェーズが 1 件作られる', async () => {
    const user = userEvent.setup()
    render(<Controlled initial="" />)
    await user.click(screen.getByLabelText('フェーズ分割を有効にする'))
    expect(screen.getAllByTestId(/^phase-card-/)).toHaveLength(1)
  })

  it('モデルとプロンプトを編集すると値に反映される', async () => {
    const user = userEvent.setup()
    render(<Controlled initial={SAMPLE} />)
    const planCard = within(screen.getByTestId('phase-card-plan'))

    const modelInput = planCard.getByLabelText('モデル')
    await user.clear(modelInput)
    await user.type(modelInput, 'claude-sonnet-4')
    expect(modelInput).toHaveValue('claude-sonnet-4')

    const promptInput = planCard.getByLabelText('プロンプト')
    await user.clear(promptInput)
    await user.type(promptInput, '新しいプロンプト')
    expect(promptInput).toHaveValue('新しいプロンプト')
  })

  it('「+ フェーズを追加」で末尾にカードが増える', async () => {
    const user = userEvent.setup()
    render(<Controlled initial={SAMPLE} />)
    await user.click(screen.getByRole('button', { name: '+ フェーズを追加' }))
    expect(screen.getAllByTestId(/^phase-card-/)).toHaveLength(3)
  })

  it('削除ボタンでカードが 1 つ減る', async () => {
    const user = userEvent.setup()
    render(<Controlled initial={SAMPLE} />)
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 調査計画 を削除' }),
    )
    expect(screen.getAllByTestId(/^phase-card-/)).toHaveLength(1)
    expect(screen.getByTestId('phase-card-investigate')).toBeInTheDocument()
  })

  it('並び替えでカードの表示順が入れ替わる', async () => {
    const user = userEvent.setup()
    render(<Controlled initial={SAMPLE} />)
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 仮説の調査 を上に移動' }),
    )
    const cards = screen.getAllByTestId(/^phase-card-/)
    expect(cards.map((c) => c.dataset['testid'])).toEqual([
      'phase-card-investigate',
      'phase-card-plan',
    ])
  })

  it('errorPhaseKey と一致するカードにだけエラーを表示する', () => {
    render(<Controlled initial={SAMPLE} errorPhaseKey="investigate" />)
    const investigateCard = screen.getByTestId('phase-card-investigate')
    const planCard = screen.getByTestId('phase-card-plan')
    expect(
      investigateCard.querySelector('[data-testid="phase-error"]'),
    ).not.toBeNull()
    expect(planCard.querySelector('[data-testid="phase-error"]')).toBeNull()
  })

  it('for_each を持つフェーズは参照元フェーズの label を説明文に使う', () => {
    render(<Controlled initial={SAMPLE} />)
    expect(screen.getByText(/調査計画 が返す/)).toBeInTheDocument()
  })

  it('出力スキーマを編集すると value (YAML) の output に反映される', async () => {
    const user = userEvent.setup()
    render(<Controlled initial={SAMPLE} />)
    const planCard = within(screen.getByTestId('phase-card-plan'))

    const outputInput = planCard.getByLabelText('出力スキーマ')
    await user.type(outputInput, 'verdict:{enter}  type: string')

    expect(planCard.getByText('✓ valid')).toBeInTheDocument()
    const yamlValue = screen.getByTestId('yaml-value').textContent
    expect(parseAgentGraphPhases(yamlValue)?.[0]?.output).toEqual({
      verdict: { type: 'string' },
    })
  })

  it('不正な出力スキーマは検証エラーを表示しつつも value (YAML) には反映される', async () => {
    const user = userEvent.setup()
    render(<Controlled initial={SAMPLE} />)
    const planCard = within(screen.getByTestId('phase-card-plan'))

    const outputInput = planCard.getByLabelText('出力スキーマ')
    await user.type(outputInput, 'verdict:{enter}  enum: supported')

    expect(
      planCard.getByText(/enum は配列である必要があります/),
    ).toBeInTheDocument()
    // 構造的な issue は警告に留め、YAML として妥当な限り value への反映はブロックしない
    const yamlValue = screen.getByTestId('yaml-value').textContent
    expect(parseAgentGraphPhases(yamlValue)?.[0]?.output).toEqual({
      verdict: { enum: 'supported' },
    })
  })
})
