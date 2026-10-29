import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  parseAgentGraphPhases,
  readTaskSteps,
  StepDetail,
  TaskExecutionTree,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'
import {
  buildPhaseNodes,
  buildTraceUrl,
  findEnumBadge,
  formatDuration,
  listEnumEntries,
  listStepPayloadReferences,
  stepSubtitle,
} from '#components/strategy-shell/task-execution-tree/model'

afterEach(cleanup)

type TaskExecutionTreeProps = ComponentProps<typeof TaskExecutionTree>

// Link (ノートリンク) が親ルートを要求するため、最低限のテストルーターを噛ませる
async function renderInRouter(ui: React.ReactElement) {
  const rootRoute = createRootRoute({ component: () => ui })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([noteRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

function makeStep(
  overrides: Partial<TaskStep> & Pick<TaskStep, 'phase_key'>,
): TaskStep {
  return {
    execution_step_id: 'exec-1',
    label: '項目確認',
    model: 'simulated-model-teal',
    status: 'completed',
    started_at: '2026-08-15T00:00:00Z',
    finished_at: '2026-08-15T00:00:08Z',
    trace_id: 'trace-1',
    span_id: 'span-1',
    ...overrides,
  }
}

function makeProps(
  overrides: Partial<TaskExecutionTreeProps> = {},
): TaskExecutionTreeProps {
  return {
    steps: [],
    configPhases: [],
    ...overrides,
  }
}

function getDefaultSelectedStepOutput() {
  return {
    expanded: screen
      .getByRole('button', { name: /項目確認/ })
      .getAttribute('aria-expanded'),
    detailSections: screen
      .getAllByText(/^(input|output)$/)
      .map((element) => element.textContent),
  }
}

function readOperationStepDetailOutput(
  step: TaskStep,
  selectedPayload: unknown,
) {
  return {
    operation: screen.getByText('evaluate_saved_code').textContent,
    model: screen.getByText('—').textContent,
    definitionHeading: screen.getByText('固定したフェーズ定義').textContent,
    payloadReferences: listStepPayloadReferences(step),
    selectedPayload,
  }
}

function getTaskStepStatusLabels() {
  return [
    screen.getByText('実行中').textContent,
    screen.getByText('失敗').textContent,
  ]
}

describe('readTaskSteps', () => {
  it('keeps a step with all required fields', () => {
    const step = makeStep({ phase_key: 'plan' })
    expect(readTaskSteps([step])).toEqual([step])
  })

  it('keeps an operation step whose model is null', () => {
    const step = makeStep({
      phase_key: 'run',
      kind: 'operation',
      model: null,
    })
    expect(readTaskSteps([step])).toEqual([step])
  })

  it('accepts an optional boolean model call limit marker and rejects other values', () => {
    const flagged = {
      ...makeStep({ phase_key: 'limited' }),
      model_call_limit_reached: true,
    }
    const unflagged = makeStep({ phase_key: 'regular' })
    const invalid = {
      ...makeStep({ phase_key: 'invalid' }),
      model_call_limit_reached: 'true',
    }

    expect(readTaskSteps([flagged, unflagged, invalid])).toEqual([
      flagged,
      unflagged,
    ])
  })

  it.each([
    'phase_key',
    'label',
    'model',
    'status',
    'started_at',
    'trace_id',
    'span_id',
  ] as const)('rejects a step missing %s', (field) => {
    const full: Record<string, unknown> = { ...makeStep({ phase_key: 'plan' }) }
    const step = Object.fromEntries(
      Object.entries(full).filter(([key]) => key !== field),
    )
    expect(readTaskSteps([step])).toEqual([])
  })
})

describe('TaskExecutionTree', () => {
  it('steps が空なら何も描画しない', () => {
    const { container } = render(<TaskExecutionTree {...makeProps()} />)
    expect(container.firstChild).toBeNull()
  })

  it('steps の無い設定フェーズは待機として表示する', () => {
    render(
      <TaskExecutionTree
        {...makeProps({
          configPhases: [
            { key: 'plan', label: '準備一覧', model: 'simulated-model-amber' },
            {
              key: 'merge',
              label: '結果整理',
              model: 'simulated-model-violet',
            },
          ],
          steps: [makeStep({ phase_key: 'plan' })],
        })}
      />,
    )

    expect(screen.getByText('結果整理')).toBeInTheDocument()
    expect(screen.getByText('simulated-model-violet')).toBeInTheDocument()
    expect(screen.getByText('待機')).toBeInTheDocument()
  })

  it('item を持つステップ群は枝として表示する', () => {
    const branches = [
      makeStep({
        phase_key: 'investigate',
        item: { title: '見出しの表記ゆれ' },
        item_label: '見出しの表記ゆれ',
        status: 'completed',
      }),
      makeStep({
        phase_key: 'investigate',
        item: { title: '注記の抜け' },
        item_label: '注記の抜け',
        status: 'running',
        finished_at: undefined,
      }),
    ]

    render(<TaskExecutionTree {...makeProps({ steps: branches })} />)

    expect(
      screen.getByRole('button', { name: /見出しの表記ゆれ/ }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /注記の抜け/ }),
    ).toBeInTheDocument()
    expect(screen.getAllByText('simulated-model-teal')).toHaveLength(2)
    expect(screen.getByText('完了')).toBeInTheDocument()
    expect(screen.getByText('実行中')).toBeInTheDocument()
  })

  it('output スキーマに enum 項目があれば、行のバッジをステータス文言の代わりに enum 値で表示する', () => {
    const configPhases = [
      {
        key: 'investigate',
        label: '項目確認',
        model: 'simulated-model-teal',
        output: { result: { enum: ['consistent', 'needs_review'] } },
      },
    ]
    const branches = [
      makeStep({
        phase_key: 'investigate',
        item: { title: '見出しの表記ゆれ' },
        item_label: '見出しの表記ゆれ',
        status: 'completed',
        output: { result: 'consistent' },
      }),
    ]

    render(
      <TaskExecutionTree {...makeProps({ configPhases, steps: branches })} />,
    )

    expect(screen.getByText('consistent')).toBeInTheDocument()
    expect(screen.queryByText('完了')).not.toBeInTheDocument()
  })

  it('model call limit marker は印のある step にだけ表示する', () => {
    render(
      <TaskExecutionTree
        {...makeProps({
          steps: [
            makeStep({
              phase_key: 'limited',
              label: '架空の確認対象 A',
              model_call_limit_reached: true,
            }),
            makeStep({
              phase_key: 'regular',
              label: '架空の確認対象 B',
            }),
          ],
        })}
      />,
    )

    expect(
      screen
        .queryAllByText('上限で打ち切り')
        .map((element) => element.textContent),
    ).toEqual(['上限で打ち切り'])
  })

  it('output がまだ無いステップは enum バッジが無いのでステータス文言を表示する', () => {
    const configPhases = [
      {
        key: 'investigate',
        label: '項目確認',
        model: 'simulated-model-teal',
        output: { result: { enum: ['consistent', 'needs_review'] } },
      },
    ]
    const branches = [
      makeStep({
        phase_key: 'investigate',
        item: { title: '注記の抜け' },
        item_label: '注記の抜け',
        status: 'running',
        finished_at: undefined,
      }),
    ]

    render(
      <TaskExecutionTree {...makeProps({ configPhases, steps: branches })} />,
    )

    expect(screen.getByText('実行中')).toBeInTheDocument()
  })

  it('running ステップは2行目に "実行中…" を表示する', () => {
    const step = makeStep({ phase_key: 'investigate', status: 'running' })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)

    expect(screen.getByText('実行中…')).toBeInTheDocument()
  })

  it('failed ステップは2行目に error を表示する', () => {
    const step = makeStep({
      phase_key: 'investigate',
      status: 'failed',
      error: 'tool call timeout',
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)

    expect(screen.getByText('tool call timeout')).toBeInTheDocument()
  })

  it('completed で note_id が無いステップは2行目を出さない', () => {
    const step = makeStep({
      phase_key: 'investigate',
      status: 'completed',
      output: { result: 'needs_review' },
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)

    expect(screen.queryByTestId('step-subtitle')).not.toBeInTheDocument()
  })

  it('実行中と失敗の状態を表示する', () => {
    const runningStep = makeStep({
      phase_key: 'investigate',
      status: 'running',
    })
    const failedStep = makeStep({ phase_key: 'other', status: 'failed' })

    render(
      <TaskExecutionTree
        {...makeProps({ steps: [runningStep, failedStep] })}
      />,
    )

    expect(getTaskStepStatusLabels()).toEqual(['実行中', '失敗'])
  })

  it('defaultSelectedStepId が指定された step の詳細を初期表示する', () => {
    const step = makeStep({
      phase_key: 'investigate',
      execution_step_id: 'sample-step-001',
      item: { value: 'sample-input' },
      output: { value: 'sample-output' },
    })

    render(
      <TaskExecutionTree
        {...makeProps({ steps: [step] })}
        defaultSelectedStepId="sample-step-001"
      />,
    )

    expect(getDefaultSelectedStepOutput()).toEqual({
      expanded: 'true',
      detailSections: ['input', 'output'],
    })
  })

  it('ノードをクリックすると item/output の詳細が開閉する', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      item_label: '見出しの表記ゆれ',
      output: { result: '一致' },
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)
    const button = screen.getByRole('button', { name: /見出しの表記ゆれ/ })

    expect(screen.queryByText('input')).not.toBeInTheDocument()
    expect(screen.queryByText('output')).not.toBeInTheDocument()

    await userEvent.click(button)
    expect(screen.getByText('input')).toBeInTheDocument()
    expect(
      screen.getByText('{ "title": "見出しの表記ゆれ" }'),
    ).toBeInTheDocument()
    expect(screen.getByText('output')).toBeInTheDocument()
    expect(screen.getByText('{ "result": "一致" }')).toBeInTheDocument()

    await userEvent.click(button)
    expect(screen.queryByText('input')).not.toBeInTheDocument()
    expect(screen.queryByText('output')).not.toBeInTheDocument()
  })

  it('traceUrlTemplate があれば選択中ステップのトレースリンクを組み立てる', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      item_label: '見出しの表記ゆれ',
      trace_id: 'trace-abc',
      span_id: 'span-def',
    })

    render(
      <TaskExecutionTree
        {...makeProps({
          steps: [step],
          traceUrlTemplate:
            'https://grafana.example/trace/{trace_id}?span={span_id}',
        })}
      />,
    )
    await userEvent.click(
      screen.getByRole('button', { name: /見出しの表記ゆれ/ }),
    )

    const link = screen.getByRole('link', { name: '→ トレースを開く' })
    expect(link).toHaveAttribute(
      'href',
      'https://grafana.example/trace/trace-abc?span=span-def',
    )
  })

  it('traceUrlTemplate が無ければトレースリンクを出さない', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      item_label: '見出しの表記ゆれ',
      trace_id: 'trace-abc',
      span_id: 'span-def',
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)
    await userEvent.click(
      screen.getByRole('button', { name: /見出しの表記ゆれ/ }),
    )

    expect(
      screen.queryByRole('link', { name: '→ トレースを開く' }),
    ).not.toBeInTheDocument()
  })

  it('failed ステータスのステップを選択すると error を表示する', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '注記の抜け' },
      item_label: '注記の抜け',
      status: 'failed',
      finished_at: '2026-08-15T00:00:05Z',
      error: 'tool call timeout',
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)
    await userEvent.click(screen.getByRole('button', { name: /注記の抜け/ }))

    expect(
      screen.getByText('tool call timeout', { selector: 'pre' }),
    ).toBeInTheDocument()
  })

  it('detailPlacement が external のときクリックしてもインライン detail を出さない', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      item_label: '見出しの表記ゆれ',
      output: { result: '一致' },
    })

    render(
      <TaskExecutionTree
        {...makeProps({ steps: [step], detailPlacement: 'external' })}
      />,
    )
    await userEvent.click(
      screen.getByRole('button', { name: /見出しの表記ゆれ/ }),
    )

    expect(screen.queryByText('output')).not.toBeInTheDocument()
  })

  it('行の選択/選択解除のたびに onSelectStep を呼ぶ', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      item_label: '見出しの表記ゆれ',
    })
    const onSelectStep = vi.fn()

    render(
      <TaskExecutionTree {...makeProps({ steps: [step], onSelectStep })} />,
    )
    const button = screen.getByRole('button', { name: /見出しの表記ゆれ/ })

    await userEvent.click(button)
    expect(onSelectStep).toHaveBeenLastCalledWith({
      step,
      outputSchema: undefined,
    })

    await userEvent.click(button)
    expect(onSelectStep).toHaveBeenLastCalledWith(null)
  })

  it('選択中の step の内容が変わったら (同じ key のまま) onSelectStep が新しい内容で再度呼ばれる', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      item_label: '見出しの表記ゆれ',
      status: 'running',
      finished_at: undefined,
    })
    const onSelectStep = vi.fn()

    const { rerender } = render(
      <TaskExecutionTree {...makeProps({ steps: [step], onSelectStep })} />,
    )
    await userEvent.click(
      screen.getByRole('button', { name: /見出しの表記ゆれ/ }),
    )
    expect(onSelectStep).toHaveBeenLastCalledWith({
      step,
      outputSchema: undefined,
    })

    const updatedStep: TaskStep = {
      ...step,
      status: 'completed',
      finished_at: '2026-08-15T00:00:08Z',
      output: { result: '一致' },
    }
    rerender(
      <TaskExecutionTree
        {...makeProps({ steps: [updatedStep], onSelectStep })}
      />,
    )

    expect(onSelectStep).toHaveBeenLastCalledWith({
      step: updatedStep,
      outputSchema: undefined,
    })
  })
})

describe('StepDetail', () => {
  it('フェーズ/モデル/所要を kv ブロックに表示する', () => {
    const step = makeStep({
      phase_key: 'investigate',
      label: '項目確認',
      model: 'simulated-model-teal',
      started_at: '2026-08-15T00:00:00Z',
      finished_at: '2026-08-15T00:00:28.700Z',
    })

    render(<StepDetail step={step} />)

    expect(screen.getByText('フェーズ')).toBeInTheDocument()
    expect(screen.getByText('項目確認')).toBeInTheDocument()
    expect(screen.getByText('モデル')).toBeInTheDocument()
    expect(screen.getByText('simulated-model-teal')).toBeInTheDocument()
    expect(screen.getByText('所要')).toBeInTheDocument()
    expect(screen.getByText('28.7s')).toBeInTheDocument()
  })

  it('所要が未確定 (未完了) なら — を表示する', () => {
    const step = makeStep({
      phase_key: 'investigate',
      status: 'running',
      finished_at: undefined,
    })

    render(<StepDetail step={step} />)

    expect(screen.getByText('—')).toBeInTheDocument()
  })

  it('outputSchema の enum 項目を項目名をラベルに値とともに表示する', () => {
    const step = makeStep({
      phase_key: 'investigate',
      output: { result: 'needs_review', details: 'done' },
    })
    const outputSchema = {
      result: { enum: ['consistent', 'needs_review'] },
      details: { type: 'string' },
    }

    render(<StepDetail step={step} outputSchema={outputSchema} />)

    expect(screen.getByText('result')).toBeInTheDocument()
    expect(screen.getByText('needs_review')).toBeInTheDocument()
    expect(screen.queryByText('details')).not.toBeInTheDocument()
  })

  it('outputSchema が無ければ enum 行を出さない', () => {
    const step = makeStep({
      phase_key: 'investigate',
      output: { result: 'needs_review' },
    })

    render(<StepDetail step={step} />)

    expect(screen.queryByText('needs_review')).not.toBeInTheDocument()
  })

  it('item/output を JSON で表示する', () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      output: { result: '一致' },
    })

    render(<StepDetail step={step} />)

    expect(screen.getByText('input')).toBeInTheDocument()
    expect(
      screen.getByText('{ "title": "見出しの表記ゆれ" }'),
    ).toBeInTheDocument()
    expect(screen.getByText('output')).toBeInTheDocument()
    expect(screen.getByText('{ "result": "一致" }')).toBeInTheDocument()
  })

  it('traceUrlTemplate があればトレースリンクを組み立てる', () => {
    const step = makeStep({
      phase_key: 'investigate',
      trace_id: 'trace-abc',
      span_id: 'span-def',
    })

    render(
      <StepDetail
        step={step}
        traceUrlTemplate="https://grafana.example/trace/{trace_id}?span={span_id}"
      />,
    )

    expect(
      screen.getByRole('link', { name: '→ トレースを開く' }),
    ).toHaveAttribute(
      'href',
      'https://grafana.example/trace/trace-abc?span=span-def',
    )
  })

  it('output に note_id があればノートへのリンクを組み立てる', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      output: { result: 'needs_review', note_id: 'note-abc' },
    })

    await renderInRouter(<StepDetail step={step} />)

    expect(
      screen.getByRole('link', { name: '→ ノートを開く' }),
    ).toHaveAttribute('href', '/notes/note-abc')
  })

  it('output に note_id が無ければノートへのリンクを出さない', () => {
    const step = makeStep({
      phase_key: 'investigate',
      output: { result: 'needs_review' },
    })

    render(<StepDetail step={step} />)

    expect(
      screen.queryByRole('link', { name: '→ ノートを開く' }),
    ).not.toBeInTheDocument()
  })

  it('output の note_id が空文字列ならノートへのリンクを出さない', () => {
    const step = makeStep({
      phase_key: 'investigate',
      output: { result: 'needs_review', note_id: '' },
    })

    render(<StepDetail step={step} />)

    expect(
      screen.queryByRole('link', { name: '→ ノートを開く' }),
    ).not.toBeInTheDocument()
  })

  it('operation の null model と固定定義、payload 参照を表示する', async () => {
    const user = userEvent.setup()
    const step = makeStep({
      phase_key: 'calculate',
      kind: 'operation',
      model: null,
      input_ref: {
        payload_inputs: [
          {
            field: 'source',
            payload_id: '00000000-0000-4000-8000-000000000001',
            pointer: '/records',
            cursor: 0,
            limit: 20,
          },
        ],
      },
      output_ref: { id: '00000000-0000-4000-8000-000000000002' },
      output: {
        payloadRef: { id: '00000000-0000-4000-8000-000000000002' },
        pointer: '/value',
      },
    })
    const onSelectPayload = vi.fn()
    render(
      <StepDetail
        step={step}
        phaseDefinition={{
          key: 'calculate',
          kind: 'operation',
          operation: 'evaluate_saved_code',
          code_id: 'sample-code',
        }}
        onSelectPayload={onSelectPayload}
      />,
    )

    await user.click(
      screen.getByRole('button', { name: '→ input: sourceを開く' }),
    )
    expect(
      readOperationStepDetailOutput(step, onSelectPayload.mock.lastCall?.[0]),
    ).toEqual({
      operation: 'evaluate_saved_code',
      model: '—',
      definitionHeading: '固定したフェーズ定義',
      payloadReferences: [
        {
          payloadId: '00000000-0000-4000-8000-000000000001',
          label: 'input: source',
          pointer: '/records',
          cursor: 0,
          limit: 20,
        },
        {
          payloadId: '00000000-0000-4000-8000-000000000002',
          label: '出力',
          pointer: '/value',
        },
      ],
      selectedPayload: {
        payloadId: '00000000-0000-4000-8000-000000000001',
        label: 'input: source',
        pointer: '/records',
        cursor: 0,
        limit: 20,
      },
    })
  })
})

describe('findEnumBadge', () => {
  it('output スキーマの enum 項目に対応する値があればそれを返す', () => {
    const outputSchema = {
      result: { enum: ['consistent', 'needs_review', 'undetermined'] },
      details: { type: 'string' },
    }

    expect(findEnumBadge(outputSchema, { result: 'needs_review' })).toBe(
      'needs_review',
    )
  })

  it('output スキーマが無ければ null を返す', () => {
    expect(findEnumBadge(undefined, { result: 'needs_review' })).toBeNull()
  })

  it('output に enum 項目の値がまだ無ければ null を返す', () => {
    const outputSchema = { result: { enum: ['consistent', 'needs_review'] } }

    expect(findEnumBadge(outputSchema, undefined)).toBeNull()
    expect(findEnumBadge(outputSchema, { other: 'x' })).toBeNull()
  })

  it('enum 宣言の無い項目しか無ければ null を返す', () => {
    const outputSchema = { details: { type: 'string' } }

    expect(findEnumBadge(outputSchema, { details: 'done' })).toBeNull()
  })
})

describe('listEnumEntries', () => {
  it('enum 項目のうち値があるものすべてを項目名をラベルとして返す', () => {
    const outputSchema = {
      result: { enum: ['consistent', 'needs_review'] },
      category: { enum: ['primary', 'secondary'] },
      details: { type: 'string' },
    }
    const output = { result: 'needs_review', category: 'primary', details: 'x' }

    expect(listEnumEntries(outputSchema, output)).toEqual([
      { label: 'result', value: 'needs_review' },
      { label: 'category', value: 'primary' },
    ])
  })

  it('outputSchema が無ければ空配列を返す', () => {
    expect(listEnumEntries(undefined, { result: 'needs_review' })).toEqual([])
  })

  it('output に値がまだ無い項目は含めない', () => {
    const outputSchema = { result: { enum: ['consistent', 'needs_review'] } }

    expect(listEnumEntries(outputSchema, undefined)).toEqual([])
    expect(listEnumEntries(outputSchema, { other: 'x' })).toEqual([])
  })
})

describe('stepSubtitle', () => {
  it('running なら "実行中…" を返す', () => {
    const step = makeStep({ phase_key: 'investigate', status: 'running' })

    expect(stepSubtitle(step)).toBe('実行中…')
  })

  it('failed かつ error があれば error を返す', () => {
    const step = makeStep({
      phase_key: 'investigate',
      status: 'failed',
      error: 'tool call timeout',
    })

    expect(stepSubtitle(step)).toBe('tool call timeout')
  })

  it('failed で error が無ければ null を返す', () => {
    const step = makeStep({ phase_key: 'investigate', status: 'failed' })

    expect(stepSubtitle(step)).toBeNull()
  })

  it('completed で output に note_id があれば "ノートを作成" を返す', () => {
    const step = makeStep({
      phase_key: 'investigate',
      status: 'completed',
      output: { note_id: 'note-abc' },
    })

    expect(stepSubtitle(step)).toBe('ノートを作成')
  })

  it('completed で output に note_id が無ければ null を返す', () => {
    const step = makeStep({
      phase_key: 'investigate',
      status: 'completed',
      output: { result: 'needs_review' },
    })

    expect(stepSubtitle(step)).toBeNull()
  })
})

describe('parseAgentGraphPhases', () => {
  it('returns empty array for unset (empty string) config', () => {
    expect(parseAgentGraphPhases('')).toEqual([])
    expect(parseAgentGraphPhases('   \n')).toEqual([])
  })

  it('extracts key/label/model from valid YAML', () => {
    const yaml = [
      'phases:',
      '  - key: plan',
      '    label: 準備一覧',
      '    model: simulated-model-amber',
      '    prompt: 項目の形式を確認する',
      '  - key: investigate',
      '    label: 確認',
      '    model: simulated-model-teal',
      '    prompt: 項目の形式を確認する',
      '    for_each: plan.items',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      {
        key: 'plan',
        label: '準備一覧',
        model: 'simulated-model-amber',
        definition: {
          key: 'plan',
          label: '準備一覧',
          model: 'simulated-model-amber',
          prompt: '項目の形式を確認する',
        },
      },
      {
        key: 'investigate',
        label: '確認',
        model: 'simulated-model-teal',
        forEach: 'plan.items',
        definition: {
          key: 'investigate',
          label: '確認',
          model: 'simulated-model-teal',
          prompt: '項目の形式を確認する',
          for_each: 'plan.items',
        },
      },
    ])
  })

  it('extracts output schema when present', () => {
    const yaml = [
      'phases:',
      '  - key: investigate',
      '    label: 確認',
      '    model: simulated-model-teal',
      '    output:',
      '      result:',
      '        enum: [consistent, needs_review]',
      '      details:',
      '        type: string',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      {
        key: 'investigate',
        label: '確認',
        model: 'simulated-model-teal',
        output: {
          result: { enum: ['consistent', 'needs_review'] },
          details: { type: 'string' },
        },
        definition: {
          key: 'investigate',
          label: '確認',
          model: 'simulated-model-teal',
          output: {
            result: { enum: ['consistent', 'needs_review'] },
            details: { type: 'string' },
          },
        },
      },
    ])
  })

  it('extracts for_each references from configured phases', () => {
    const yaml = [
      'phases:',
      '  - key: inspect',
      '    label: 確認',
      '    model: simulated-model-teal',
      '    for_each: source.entries',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      {
        key: 'inspect',
        label: '確認',
        model: 'simulated-model-teal',
        forEach: 'source.entries',
        definition: {
          key: 'inspect',
          label: '確認',
          model: 'simulated-model-teal',
          for_each: 'source.entries',
        },
      },
    ])
  })

  it('returns empty array for invalid YAML', () => {
    expect(parseAgentGraphPhases('phases: [')).toEqual([])
  })

  it('returns empty array when phases is missing or not an array', () => {
    expect(parseAgentGraphPhases('foo: bar')).toEqual([])
    expect(parseAgentGraphPhases('phases: 1')).toEqual([])
  })

  it('extracts operation order and nullable model from a fixed graph', () => {
    const yaml = [
      'phases:',
      '  - key: calculate',
      '    label: 計算',
      '    kind: operation',
      '    operation: evaluate_saved_code',
      '    code_id: sample-code',
      '    model: null',
      '    input:',
      '      source:',
      '        prior_output: { phase: prepare, pointer: /records, limit: 20 }',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      {
        key: 'calculate',
        label: '計算',
        kind: 'operation',
        operation: 'evaluate_saved_code',
        codeId: 'sample-code',
        model: null,
        definition: {
          key: 'calculate',
          label: '計算',
          kind: 'operation',
          operation: 'evaluate_saved_code',
          code_id: 'sample-code',
          model: null,
          input: {
            source: {
              prior_output: {
                phase: 'prepare',
                pointer: '/records',
                limit: 20,
              },
            },
          },
        },
      },
    ])
  })

  it('skips phase entries missing required string fields', () => {
    const yaml = [
      'phases:',
      '  - key: plan',
      '    label: 準備一覧',
      '    model: simulated-model-amber',
      '  - key: broken',
      '    label: 壊れた設定',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      {
        key: 'plan',
        label: '準備一覧',
        model: 'simulated-model-amber',
        definition: {
          key: 'plan',
          label: '準備一覧',
          model: 'simulated-model-amber',
        },
      },
    ])
  })
})

describe('buildPhaseNodes', () => {
  it('returns empty array when steps is empty, even with configured phases', () => {
    const configPhases = [
      { key: 'plan', label: '準備一覧', model: 'simulated-model-amber' },
    ]
    expect(buildPhaseNodes(configPhases, [])).toEqual([])
  })

  it('marks configured phases with no steps as pending', () => {
    const configPhases = [
      { key: 'plan', label: '準備一覧', model: 'simulated-model-amber' },
      { key: 'merge', label: '結果整理', model: 'simulated-model-violet' },
    ]
    const steps = [makeStep({ phase_key: 'plan' })]

    expect(buildPhaseNodes(configPhases, steps)).toEqual([
      { kind: 'single', key: 'plan', step: steps[0] },
      {
        kind: 'pending',
        key: 'merge',
        label: '結果整理',
        model: 'simulated-model-violet',
      },
    ])
  })

  const configPhases = [
    { key: 'source', label: '入力', model: 'simulated-model-teal' },
    {
      key: 'inspect',
      label: '確認',
      model: 'simulated-model-teal',
      forEach: 'source.entries',
    },
    { key: 'details', label: '整理', model: 'simulated-model-teal' },
  ]
  const steps = [
    makeStep({ phase_key: 'source' }),
    makeStep({ phase_key: 'details' }),
  ]

  it.each([
    {
      taskCompleted: false,
      expected: [
        { kind: 'single', key: 'source', step: steps[0] },
        {
          kind: 'pending',
          key: 'inspect',
          label: '確認',
          model: 'simulated-model-teal',
        },
        { kind: 'single', key: 'details', step: steps[1] },
      ],
    },
    {
      taskCompleted: true,
      expected: [
        { kind: 'single', key: 'source', step: steps[0] },
        {
          kind: 'empty',
          key: 'inspect',
          label: '確認',
          model: 'simulated-model-teal',
        },
        { kind: 'single', key: 'details', step: steps[1] },
      ],
    },
  ])(
    'renders empty for_each phase correctly when taskCompleted=$taskCompleted',
    ({ taskCompleted, expected }) => {
      expect(buildPhaseNodes(configPhases, steps, taskCompleted)).toEqual(
        expected,
      )
    },
  )

  it('groups steps carrying an item into a branch node', () => {
    const configPhases = [
      { key: 'investigate', label: '確認', model: 'simulated-model-teal' },
    ]
    const branchA = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
      item_label: '見出しの表記ゆれ',
      started_at: '2026-08-15T00:00:01Z',
    })
    const branchB = makeStep({
      phase_key: 'investigate',
      item: { title: '注記の抜け' },
      item_label: '注記の抜け',
      status: 'running',
      finished_at: undefined,
      started_at: '2026-08-15T00:00:00Z',
    })

    expect(buildPhaseNodes(configPhases, [branchA, branchB])).toEqual([
      { kind: 'branch', key: 'investigate', branches: [branchB, branchA] },
    ])
  })

  it('configPhases の output をノードの outputSchema として運ぶ', () => {
    const outputSchema = { result: { enum: ['consistent', 'needs_review'] } }
    const configPhases = [
      {
        key: 'investigate',
        label: '確認',
        model: 'simulated-model-teal',
        output: outputSchema,
      },
    ]
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '見出しの表記ゆれ' },
    })

    expect(buildPhaseNodes(configPhases, [step])).toEqual([
      { kind: 'branch', key: 'investigate', branches: [step], outputSchema },
    ])
  })

  it('appends phase_keys absent from config in first-seen order', () => {
    const staleFirst = makeStep({
      phase_key: 'stale-first',
      started_at: '2026-08-15T00:00:00Z',
    })
    const staleSecond = makeStep({
      phase_key: 'stale-second',
      started_at: '2026-08-15T00:00:01Z',
    })

    expect(buildPhaseNodes([], [staleSecond, staleFirst])).toEqual([
      { kind: 'single', key: 'stale-first', step: staleFirst },
      { kind: 'single', key: 'stale-second', step: staleSecond },
    ])
  })
})

describe('formatDuration', () => {
  it('returns null while the step has not finished', () => {
    expect(formatDuration('2026-08-15T00:00:00Z', null)).toBeNull()
    expect(formatDuration('2026-08-15T00:00:00Z', undefined)).toBeNull()
  })

  it('formats the elapsed seconds to one decimal place', () => {
    expect(
      formatDuration('2026-08-15T00:00:00.000Z', '2026-08-15T00:00:12.400Z'),
    ).toBe('12.4s')
  })

  it('returns null for unparseable timestamps', () => {
    expect(formatDuration('not-a-date', '2026-08-15T00:00:00Z')).toBeNull()
  })
})

describe('buildTraceUrl', () => {
  it('returns null when no template is configured', () => {
    expect(buildTraceUrl(undefined, 'trace-1', 'span-1')).toBeNull()
    expect(buildTraceUrl('', 'trace-1', 'span-1')).toBeNull()
  })

  it('substitutes trace_id and span_id placeholders', () => {
    expect(
      buildTraceUrl(
        'https://grafana.example/trace/{trace_id}?span={span_id}',
        'trace-1',
        'span-1',
      ),
    ).toBe('https://grafana.example/trace/trace-1?span=span-1')
  })

  it('substitutes an empty span_id when absent', () => {
    expect(
      buildTraceUrl(
        'https://grafana.example/trace/{trace_id}?span={span_id}',
        'trace-1',
        null,
      ),
    ).toBe('https://grafana.example/trace/trace-1?span=')
  })
})
