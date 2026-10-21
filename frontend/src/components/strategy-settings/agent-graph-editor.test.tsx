import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ComponentProps, ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { parseAgentGraphPhases } from '#components/strategy-settings/agent-graph/document'
import { AgentGraphEditor } from '#components/strategy-settings/agent-graph-editor'
import { fetchClient } from '#lib/api/client'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)
// monaco-setup は実物の monaco-editor を import するため jsdom では評価できない
vi.mock('#components/indicators/monaco-setup', () => ({}))

afterEach(cleanup)

// 有効なフェーズ YAML ではない (parseAgentGraphPhases が null を返す) 単純な文字列。
// dirty/save/beforeunload のような view に依存しない機構をテストするときは、これを使って
// 常に YAML ビューを表示させる (form ビューがデフォルトになると agent_graph エディタが
// 画面から消えてテストの前提が崩れるため)。
const NOT_PHASE_YAML = 'sample'

const SAMPLE_PHASES = `phases:
  - key: alpha
    label: サンプル
    model: fictional-model-orbit
    prompt: sample prompt
`

// AgentGraphForm (form ビュー) が $api.useQuery でモデル/tool/skills 一覧を取得するため、
// このファイルのテストは view の切り替え挙動だけを見ていれば十分 (空データで問題ない)
function installMiddleware() {
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      if (/\/api\/agent-models(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ models: [] }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/agent-tools(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ tools: [] }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/agent-configs\/[^/]+\/skills(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ skills: {} }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      throw new Error(`unmocked request: ${request.method} ${url}`)
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

let ejectMiddleware: (() => void) | null = null

function renderEditor(
  props: Omit<ComponentProps<typeof AgentGraphEditor>, 'purpose'>,
) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  return render(<AgentGraphEditor purpose="sample-purpose" {...props} />, {
    wrapper: Wrapper,
  })
}

beforeEach(() => {
  ejectMiddleware = installMiddleware()
})

afterEach(() => {
  ejectMiddleware?.()
  ejectMiddleware = null
})

describe('AgentGraphEditor', () => {
  it('initialValue と一致する間は保存ボタンが disabled で dirty 表示も出ない', () => {
    renderEditor({ initialValue: NOT_PHASE_YAML, onSave: () => {} })
    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled()
    expect(screen.queryByTestId('dirty-indicator')).toBeNull()
  })

  it('編集すると dirty になり、保存ボタンが押せる', async () => {
    const user = userEvent.setup()
    renderEditor({ initialValue: NOT_PHASE_YAML, onSave: () => {} })
    const editor = screen.getByLabelText('agent_graph')
    await user.type(editor, '\n')
    expect(screen.getByTestId('dirty-indicator')).toBeInTheDocument()
    expect(screen.getByRole('button', { name: '保存' })).not.toBeDisabled()
  })

  it('保存ボタンを押すと現在の値で onSave が呼ばれる', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()
    renderEditor({ initialValue: 'a', onSave })
    const editor = screen.getByLabelText('agent_graph')
    // clear() で一時的に空文字列を経由すると「フェーズ分割 off」の正当な値として
    // フォームビューに切り替わり YAML エディタが外れてしまうため、末尾に追記する
    await user.type(editor, 'b')
    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(onSave).toHaveBeenCalledWith('ab')
  })

  it('isSaving 中は保存ボタンが「保存中…」表示で disabled になる', async () => {
    const user = userEvent.setup()
    renderEditor({ initialValue: 'a', onSave: () => {}, isSaving: true })
    const editor = screen.getByLabelText('agent_graph')
    await user.type(editor, 'X')
    expect(screen.getByRole('button', { name: '保存中…' })).toBeDisabled()
  })

  it('saveError があるとエラーメッセージを描画する', () => {
    renderEditor({
      initialValue: 'a',
      onSave: () => {},
      saveError: '保存に失敗しました',
    })
    expect(screen.getByText('保存に失敗しました')).toBeInTheDocument()
  })

  it('dirty 状態で発火した beforeunload イベントを preventDefault する', async () => {
    const user = userEvent.setup()
    renderEditor({ initialValue: 'a', onSave: () => {} })
    await user.type(screen.getByLabelText('agent_graph'), 'X')

    const ev = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(true)
  })

  it('クリーン状態の beforeunload イベントは preventDefault しない', () => {
    renderEditor({ initialValue: 'a', onSave: () => {} })

    const ev = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(false)
  })

  it('編集していないときに親から initialValue が更新されたら追従し dirty 表示は出さない', () => {
    const { rerender } = renderEditor({ initialValue: 'A', onSave: () => {} })
    expect(screen.getByLabelText('agent_graph')).toHaveValue('A')

    rerender(
      <AgentGraphEditor
        purpose="sample-purpose"
        initialValue="B"
        onSave={() => {}}
      />,
    )
    expect(screen.getByLabelText('agent_graph')).toHaveValue('B')
    expect(screen.queryByTestId('dirty-indicator')).toBeNull()
  })

  it('編集中に親から initialValue が更新されてもユーザーの draft を上書きしない', async () => {
    const user = userEvent.setup()
    const { rerender } = renderEditor({ initialValue: 'A', onSave: () => {} })
    const editor = screen.getByLabelText('agent_graph')
    // clear() で一時的に空文字列を経由すると「フェーズ分割 off」の正当な値として
    // フォームビューに切り替わり YAML エディタが外れてしまうため、末尾に追記する
    await user.type(editor, 'draft')

    rerender(
      <AgentGraphEditor
        purpose="sample-purpose"
        initialValue="B"
        onSave={() => {}}
      />,
    )
    expect(screen.getByLabelText('agent_graph')).toHaveValue('Adraft')
  })

  it('有効なフェーズ YAML はデフォルトでフォームビューを表示する', () => {
    renderEditor({ initialValue: SAMPLE_PHASES, onSave: () => {} })
    expect(screen.getByTestId('phase-card-alpha')).toBeInTheDocument()
    expect(screen.queryByLabelText('agent_graph')).toBeNull()
  })

  it('YAML チップを押すと生 YAML エディタに切り替わる', async () => {
    const user = userEvent.setup()
    renderEditor({ initialValue: SAMPLE_PHASES, onSave: () => {} })
    await user.click(screen.getByRole('button', { name: 'YAML' }))
    expect(screen.getByLabelText('agent_graph')).toHaveValue(SAMPLE_PHASES)
    expect(screen.queryByTestId('phase-card-alpha')).toBeNull()
  })

  it('壊れた YAML では YAML のみ選べ、エディタと警告が表示される', () => {
    renderEditor({ initialValue: 'phases: [', onSave: () => {} })
    const viewButtons = within(screen.getByRole('group', { name: '表示形式' }))
      .getAllByRole('button')
      .map((button) => ({
        name: button.textContent.trim(),
        pressed: button.getAttribute('aria-pressed'),
      }))
    function readActualState() {
      return {
        viewButtons,
        yamlEditorVisible: screen.queryByLabelText('agent_graph') != null,
        formUnavailableNoticeVisible:
          screen.queryByTestId('form-unavailable-notice') != null,
      }
    }
    expect(readActualState()).toEqual({
      viewButtons: [{ name: 'YAML', pressed: 'true' }],
      yamlEditorVisible: true,
      formUnavailableNoticeVisible: true,
    })
  })

  it('saveError からフェーズ key を抽出し、該当カードにエラーを表示する', () => {
    renderEditor({
      initialValue: SAMPLE_PHASES,
      onSave: () => {},
      saveError: 'phase key "alpha" is duplicated',
    })
    expect(
      screen
        .getByTestId('phase-card-alpha')
        .querySelector('[data-testid="phase-error"]'),
    ).not.toBeNull()
  })

  it('フォームで編集した内容 (label・output 含む) が YAML ビューに反映される', async () => {
    const user = userEvent.setup()
    renderEditor({ initialValue: SAMPLE_PHASES, onSave: () => {} })

    const alphaCard = within(screen.getByTestId('phase-card-alpha'))
    const labelInput = alphaCard.getByLabelText('フェーズ名')
    await user.type(labelInput, 'X')

    const outputInput = alphaCard.getByLabelText('出力スキーマ')
    await user.type(outputInput, 'x:{enter}  type: string')

    await user.click(screen.getByRole('button', { name: 'YAML' }))
    const yamlEditor = screen.getByLabelText<HTMLTextAreaElement>('agent_graph')
    expect(parseAgentGraphPhases(yamlEditor.value)).toEqual([
      {
        key: 'alpha',
        label: 'サンプルX',
        model: 'fictional-model-orbit',
        prompt: 'sample prompt',
        forEach: undefined,
        labelField: undefined,
        maxParallel: undefined,
        skills: [],
        tools: undefined,
        output: { x: { type: 'string' } },
      },
    ])
  })

  it('YAML ビューからフォームに戻しても編集内容 (label・output 含む) が保たれる', async () => {
    const user = userEvent.setup()
    renderEditor({ initialValue: SAMPLE_PHASES, onSave: () => {} })

    const alphaCard = within(screen.getByTestId('phase-card-alpha'))
    const labelInput = alphaCard.getByLabelText('フェーズ名')
    await user.type(labelInput, 'X')

    const outputInput = alphaCard.getByLabelText('出力スキーマ')
    await user.type(outputInput, 'x:{enter}  type: string')

    await user.click(screen.getByRole('button', { name: 'YAML' }))
    await user.click(screen.getByRole('button', { name: 'フォーム' }))

    const alphaCardAfter = within(screen.getByTestId('phase-card-alpha'))
    expect(alphaCardAfter.getByLabelText('フェーズ名')).toHaveValue('サンプルX')
    expect(alphaCardAfter.getByText('✓ valid')).toBeInTheDocument()
  })
})
