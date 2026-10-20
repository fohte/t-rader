import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { useEffect, useRef, useState } from 'react'
import {
  afterEach,
  beforeAll,
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest'

import { AgentGraphForm } from '#components/strategy-settings/agent-graph/agent-graph-form'
import { parseAgentGraphPhases } from '#components/strategy-settings/agent-graph/document'
import { fetchClient } from '#lib/api/client'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)
// monaco-setup は実物の monaco-editor を import するため jsdom では評価できない
vi.mock('#components/indicators/monaco-setup', () => ({}))

beforeAll(() => {
  // Radix Select が内部で参照する API は jsdom に無いためポリフィルする
  window.HTMLElement.prototype.hasPointerCapture = () => false
  window.HTMLElement.prototype.scrollIntoView = () => {}
})

const SAMPLE = `phases:
  - key: plan
    label: 見出し案
    model: fictional-model-orbit
    prompt: テーマを読みやすい節に分ける
    output:
      sections:
        type: array
        items:
          title: { type: string }
      appendices:
        type: array
        items:
          title: { type: string }
  - key: compose
    label: 節の作成
    model: fictional-model-lantern
    for_each: plan.sections
    label_field: title
    max_parallel: 4
    prompt: 割り当てられた見出しに沿って、説明文を作成する。
`

const MODELS = [
  {
    id: 'fictional-model-orbit',
    providers: ['fictional-lab'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: true,
  },
  {
    id: 'fictional-model-maple',
    providers: ['fictional-lab'],
    max_input_tokens: null,
    max_output_tokens: null,
    supports_reasoning: false,
  },
]
const TOOLS = [
  { name: 'find_reference', description: null },
  { name: 'read_reference', description: null },
]
const SKILLS = {
  outline_guide: '# outline_guide\n',
  style_guide: '# style_guide\n',
}

function installMiddleware() {
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      if (/\/api\/agent-models(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ models: MODELS }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/agent-tools(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ tools: TOOLS }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }
      if (/\/api\/agent-configs\/[^/]+\/skills(\?|$)/.test(url)) {
        return new Response(JSON.stringify({ skills: SKILLS }), {
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
        purpose="explore"
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

function renderForm(props: { initial: string; errorPhaseKey?: string | null }) {
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  return render(<Controlled {...props} />, { wrapper: Wrapper })
}

beforeEach(() => {
  ejectMiddleware = installMiddleware()
})

afterEach(() => {
  cleanup()
  ejectMiddleware?.()
  ejectMiddleware = null
})

describe('AgentGraphForm', () => {
  it('value が空文字列なら分割トグルが off で表示され、カードは出ない', () => {
    renderForm({ initial: '' })
    expect(screen.getByLabelText('フェーズ分割を有効にする')).not.toBeChecked()
    expect(screen.queryByTestId('phase-card-plan')).toBeNull()
  })

  it('value がフェーズを含めば分割トグルが on で、フェーズカードを表示する', () => {
    renderForm({ initial: SAMPLE })
    expect(screen.getByLabelText('フェーズ分割を有効にする')).toBeChecked()
    expect(screen.getByTestId('phase-card-plan')).toBeInTheDocument()
    expect(screen.getByTestId('phase-card-compose')).toBeInTheDocument()
  })

  it('トグルを off にすると値が空文字列になり、on に戻すと復元される', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })

    await user.click(screen.getByLabelText('フェーズ分割を有効にする'))
    expect(screen.queryByTestId('phase-card-plan')).toBeNull()

    await user.click(screen.getByLabelText('フェーズ分割を有効にする'))
    expect(screen.getByTestId('phase-card-plan')).toBeInTheDocument()
    expect(screen.getByTestId('phase-card-compose')).toBeInTheDocument()
  })

  it('未設定の状態でトグルを on にすると最小構成のフェーズが 1 件作られる', async () => {
    const user = userEvent.setup()
    renderForm({ initial: '' })
    await user.click(screen.getByLabelText('フェーズ分割を有効にする'))
    expect(screen.getAllByTestId(/^phase-card-/)).toHaveLength(1)
  })

  it('フェーズ名を編集すると値に反映される', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const planCard = within(screen.getByTestId('phase-card-plan'))

    const labelInput = planCard.getByLabelText('フェーズ名')
    await user.clear(labelInput)
    await user.type(labelInput, '新しい見出し案')
    expect(labelInput).toHaveValue('新しい見出し案')

    const yamlValue = screen.getByTestId('yaml-value').textContent
    expect(parseAgentGraphPhases(yamlValue)?.[0]).toEqual({
      key: 'plan',
      label: '新しい見出し案',
      model: 'fictional-model-orbit',
      prompt: 'テーマを読みやすい節に分ける',
      forEach: undefined,
      labelField: undefined,
      maxParallel: undefined,
      skills: [],
      tools: undefined,
      output: {
        sections: {
          type: 'array',
          items: { title: { type: 'string' } },
        },
        appendices: {
          type: 'array',
          items: { title: { type: 'string' } },
        },
      },
    })
  })

  it('プロンプトを編集すると値に反映される', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const planCard = within(screen.getByTestId('phase-card-plan'))

    const promptInput = planCard.getByLabelText('プロンプト')
    await user.clear(promptInput)
    await user.type(promptInput, '新しいプロンプト')
    expect(promptInput).toHaveValue('新しいプロンプト')
  })

  it('モデルを select から選ぶと値に反映される', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const planCard = within(screen.getByTestId('phase-card-plan'))

    const trigger = await planCard.findByRole('combobox', { name: 'モデル' })
    expect(trigger).toHaveTextContent('fictional-model-orbit')

    await user.click(trigger)
    await user.click(
      await screen.findByRole('option', { name: 'fictional-model-maple' }),
    )
    expect(trigger).toHaveTextContent('fictional-model-maple')
  })

  it('tool チップを追加・削除でき、絞り込みの有無を tools キーの有無で表現する', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const planCard = within(screen.getByTestId('phase-card-plan'))

    // 初期状態は tools 省略 (全 tool 使用可)
    expect(await planCard.findByText('すべての tool')).toBeInTheDocument()

    await user.click(planCard.getByRole('button', { name: '絞り込む' }))
    expect(planCard.queryByText('すべての tool')).toBeNull()

    await user.click(planCard.getByRole('button', { name: 'tool を追加' }))
    await user.click(
      await screen.findByRole('button', { name: 'find_reference' }),
    )
    expect(planCard.getByText('find_reference')).toBeInTheDocument()

    await user.click(
      planCard.getByRole('button', { name: 'tool "find_reference" を外す' }),
    )
    expect(planCard.queryByText('find_reference')).toBeNull()

    await user.click(planCard.getByRole('button', { name: '全 tool に戻す' }))
    expect(await planCard.findByText('すべての tool')).toBeInTheDocument()
  })

  it('skill チップを追加・削除できる', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const planCard = within(screen.getByTestId('phase-card-plan'))

    // tools が未設定 (絞り込む前) の間は「+ 追加」ボタンは skills 側だけに存在する
    await user.click(
      await planCard.findByRole('button', { name: 'skill を追加' }),
    )
    await user.click(
      await screen.findByRole('button', { name: 'outline_guide' }),
    )
    expect(planCard.getByText('outline_guide')).toBeInTheDocument()

    await user.click(
      planCard.getByRole('button', { name: 'skill "outline_guide" を外す' }),
    )
    expect(planCard.queryByText('outline_guide')).toBeNull()
  })

  it('「+ フェーズを追加」で末尾にカードが増える', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    await user.click(screen.getByRole('button', { name: '+ フェーズを追加' }))
    expect(screen.getAllByTestId(/^phase-card-/)).toHaveLength(3)
  })

  it('削除ボタンでカードが 1 つ減る', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 見出し案 を削除' }),
    )
    expect(screen.getAllByTestId(/^phase-card-/)).toHaveLength(1)
    expect(screen.getByTestId('phase-card-compose')).toBeInTheDocument()
  })

  it('並び替えでカードの表示順が入れ替わる', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    await user.click(
      screen.getByRole('button', { name: 'フェーズ 節の作成 を上に移動' }),
    )
    const cards = screen.getAllByTestId(/^phase-card-/)
    expect(cards.map((c) => c.dataset['testid'])).toEqual([
      'phase-card-compose',
      'phase-card-plan',
    ])
  })

  it('errorPhaseKey と一致するカードにだけエラーを表示する', () => {
    renderForm({ initial: SAMPLE, errorPhaseKey: 'compose' })
    const composeCard = screen.getByTestId('phase-card-compose')
    const planCard = screen.getByTestId('phase-card-plan')
    expect(
      composeCard.querySelector('[data-testid="phase-error"]'),
    ).not.toBeNull()
    expect(planCard.querySelector('[data-testid="phase-error"]')).toBeNull()
  })

  it('for_each を持つフェーズは参照元フェーズの label を説明文に使う', () => {
    renderForm({ initial: SAMPLE })
    expect(screen.getByText(/見出し案 が返す/)).toBeInTheDocument()
  })

  it('for_each を持たないフェーズにはノード名・並列上限を表示しない', () => {
    renderForm({ initial: SAMPLE })
    const planCard = within(screen.getByTestId('phase-card-plan'))
    expect(planCard.queryByLabelText('ノード名')).toBeNull()
    expect(planCard.queryByLabelText('並列上限')).toBeNull()
  })

  it('for_each を持つフェーズにはノード名・並列上限を表示する', () => {
    renderForm({ initial: SAMPLE })
    const composeCard = within(screen.getByTestId('phase-card-compose'))
    expect(composeCard.getByLabelText('実行回数')).toBeInTheDocument()
    expect(composeCard.getByLabelText('ノード名')).toBeInTheDocument()
    expect(composeCard.getByLabelText('並列上限')).toBeInTheDocument()
  })

  it('並列上限は既存の値を反映する', () => {
    renderForm({ initial: SAMPLE })
    const composeCard = within(screen.getByTestId('phase-card-compose'))
    expect(composeCard.getByLabelText('並列上限')).toHaveValue(4)
  })

  it('並列上限を編集すると値に反映される', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const composeCard = within(screen.getByTestId('phase-card-compose'))
    const maxParallelInput = composeCard.getByLabelText('並列上限')
    await user.clear(maxParallelInput)
    await user.type(maxParallelInput, '8')
    expect(maxParallelInput).toHaveValue(8)
  })

  it('実行回数を別の参照先に切り替えると、旧参照先の items に紐づくノード名の選択がリセットされる', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const composeCard = within(screen.getByTestId('phase-card-compose'))
    expect(composeCard.getByLabelText('ノード名')).toHaveTextContent('title')

    await user.click(composeCard.getByLabelText('実行回数'))
    await user.click(
      await screen.findByRole('option', { name: /appendices\[\] の要素ごと/ }),
    )

    expect(
      within(screen.getByTestId('phase-card-compose')).getByLabelText(
        'ノード名',
      ),
    ).toHaveTextContent('(選択肢なし)')
  })

  it('出力スキーマを編集すると value (YAML) の output に反映される', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const composeCard = within(screen.getByTestId('phase-card-compose'))

    const outputInput = composeCard.getByLabelText('出力スキーマ')
    await user.type(outputInput, 'abstract:{enter}  type: string')

    expect(composeCard.getByText('✓ valid')).toBeInTheDocument()
    const yamlValue = screen.getByTestId('yaml-value').textContent
    expect(parseAgentGraphPhases(yamlValue)?.[1]?.output).toEqual({
      abstract: { type: 'string' },
    })
  })

  it('不正な出力スキーマは検証エラーを表示しつつも value (YAML) には反映される', async () => {
    const user = userEvent.setup()
    renderForm({ initial: SAMPLE })
    const composeCard = within(screen.getByTestId('phase-card-compose'))

    const outputInput = composeCard.getByLabelText('出力スキーマ')
    await user.type(outputInput, 'abstract:{enter}  enum: brief')

    expect(
      composeCard.getByText(/enum は配列である必要があります/),
    ).toBeInTheDocument()
    // 構造的な issue は警告に留め、YAML として妥当な限り value への反映はブロックしない
    const yamlValue = screen.getByTestId('yaml-value').textContent
    expect(parseAgentGraphPhases(yamlValue)?.[1]?.output).toEqual({
      abstract: { enum: 'brief' },
    })
  })
})
