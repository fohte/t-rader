import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AgentGraphEditor } from '#components/strategy-settings/agent-graph-editor'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)

afterEach(cleanup)

// 有効なフェーズ YAML ではない (parseAgentGraphPhases が null を返す) 単純な文字列。
// dirty/save/beforeunload のような view に依存しない機構をテストするときは、これを使って
// 常に YAML ビューを表示させる (form ビューがデフォルトになると agent_graph エディタが
// 画面から消えてテストの前提が崩れるため)。
const NOT_PHASE_YAML = 'sample'

const SAMPLE_PHASES = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    prompt: 仮説を立てよ
`

describe('AgentGraphEditor', () => {
  it('initialValue と一致する間は保存ボタンが disabled で dirty 表示も出ない', () => {
    render(<AgentGraphEditor initialValue={NOT_PHASE_YAML} onSave={() => {}} />)
    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled()
    expect(screen.queryByTestId('dirty-indicator')).toBeNull()
  })

  it('編集すると dirty になり、保存ボタンが押せる', async () => {
    const user = userEvent.setup()
    render(<AgentGraphEditor initialValue={NOT_PHASE_YAML} onSave={() => {}} />)
    const editor = screen.getByLabelText('agent_graph')
    await user.type(editor, '\n')
    expect(screen.getByTestId('dirty-indicator')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保存' })).not.toBeDisabled()
  })

  it('保存ボタンを押すと現在の値で onSave が呼ばれる', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()
    render(<AgentGraphEditor initialValue="a" onSave={onSave} />)
    const editor = screen.getByLabelText('agent_graph')
    // clear() で一時的に空文字列を経由すると「フェーズ分割 off」の正当な値として
    // フォームビューに切り替わり YAML エディタが外れてしまうため、末尾に追記する
    await user.type(editor, 'b')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(onSave).toHaveBeenCalledWith('ab')
  })

  it('isSaving 中は保存ボタンが「保存中…」表示で disabled になる', async () => {
    const user = userEvent.setup()
    render(<AgentGraphEditor initialValue="a" onSave={() => {}} isSaving />)
    const editor = screen.getByLabelText('agent_graph')
    await user.type(editor, 'X')
    expect(screen.getByRole('button', { name: '保存中…' })).toBeDisabled()
  })

  it('saveError があるとエラーメッセージを描画する', () => {
    render(
      <AgentGraphEditor
        initialValue="a"
        onSave={() => {}}
        saveError="保存に失敗しました"
      />,
    )
    expect(screen.getByText('保存に失敗しました')).toBeInTheDocument()
  })

  it('dirty 状態で発火した beforeunload イベントを preventDefault する', async () => {
    const user = userEvent.setup()
    render(<AgentGraphEditor initialValue="a" onSave={() => {}} />)
    await user.type(screen.getByLabelText('agent_graph'), 'X')

    const ev = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(true)
  })

  it('クリーン状態の beforeunload イベントは preventDefault しない', () => {
    render(<AgentGraphEditor initialValue="a" onSave={() => {}} />)

    const ev = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(false)
  })

  it('編集していないときに親から initialValue が更新されたら追従し dirty 表示は出さない', () => {
    const { rerender } = render(
      <AgentGraphEditor initialValue="A" onSave={() => {}} />,
    )
    expect(screen.getByLabelText('agent_graph')).toHaveValue('A')

    rerender(<AgentGraphEditor initialValue="B" onSave={() => {}} />)
    expect(screen.getByLabelText('agent_graph')).toHaveValue('B')
    expect(screen.queryByTestId('dirty-indicator')).toBeNull()
  })

  it('編集中に親から initialValue が更新されてもユーザーの draft を上書きしない', async () => {
    const user = userEvent.setup()
    const { rerender } = render(
      <AgentGraphEditor initialValue="A" onSave={() => {}} />,
    )
    const editor = screen.getByLabelText('agent_graph')
    // clear() で一時的に空文字列を経由すると「フェーズ分割 off」の正当な値として
    // フォームビューに切り替わり YAML エディタが外れてしまうため、末尾に追記する
    await user.type(editor, 'draft')

    rerender(<AgentGraphEditor initialValue="B" onSave={() => {}} />)
    expect(screen.getByLabelText('agent_graph')).toHaveValue('Adraft')
  })

  it('有効なフェーズ YAML はデフォルトでフォームビューを表示する', () => {
    render(<AgentGraphEditor initialValue={SAMPLE_PHASES} onSave={() => {}} />)
    expect(screen.getByTestId('phase-card-plan')).toBeInTheDocument()
    expect(screen.queryByLabelText('agent_graph')).toBeNull()
  })

  it('YAML チップを押すと生 YAML エディタに切り替わる', async () => {
    const user = userEvent.setup()
    render(<AgentGraphEditor initialValue={SAMPLE_PHASES} onSave={() => {}} />)
    await user.click(screen.getByRole('button', { name: 'YAML' }))
    expect(screen.getByLabelText('agent_graph')).toHaveValue(SAMPLE_PHASES)
    expect(screen.queryByTestId('phase-card-plan')).toBeNull()
  })

  it('壊れた YAML はフォームチップが disabled になり、常に YAML ビューになる', () => {
    render(<AgentGraphEditor initialValue="phases: [" onSave={() => {}} />)
    expect(screen.getByRole('button', { name: 'フォーム' })).toBeDisabled()
    expect(screen.getByLabelText('agent_graph')).toBeInTheDocument()
    expect(screen.getByTestId('form-unavailable-notice')).toBeInTheDocument()
  })

  it('saveError からフェーズ key を抽出し、該当カードにエラーを表示する', () => {
    render(
      <AgentGraphEditor
        initialValue={SAMPLE_PHASES}
        onSave={() => {}}
        saveError='phase key "plan" is duplicated'
      />,
    )
    expect(
      screen
        .getByTestId('phase-card-plan')
        .querySelector('[data-testid="phase-error"]'),
    ).not.toBeNull()
  })
})
