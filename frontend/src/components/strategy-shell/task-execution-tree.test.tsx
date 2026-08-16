import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  buildPhaseNodes,
  buildTraceUrl,
  findEnumBadge,
  formatDuration,
  isTaskStep,
  parseAgentGraphPhases,
  StepDetail,
  TaskExecutionTree,
  type TaskExecutionTreeProps,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'

afterEach(cleanup)

// Link (ノートリンク) が親ルートを要求するため、最低限のテストルーターを噛ませる
function renderInRouter(ui: React.ReactElement) {
  const rootRoute = createRootRoute({ component: () => ui })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([noteRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  return render(<RouterProvider router={router} />)
}

function makeStep(
  overrides: Partial<TaskStep> & Pick<TaskStep, 'phase_key'>,
): TaskStep {
  return {
    label: '仮説の調査',
    model: 'deepseek-v4-flash',
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
    strategyId: 'strategy-1',
    ...overrides,
  }
}

describe('isTaskStep', () => {
  it('accepts a step with all required fields', () => {
    expect(isTaskStep(makeStep({ phase_key: 'plan' }))).toBe(true)
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
    expect(isTaskStep(step)).toBe(false)
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
            { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
            { key: 'merge', label: '統合', model: 'claude-sonnet-4' },
          ],
          steps: [makeStep({ phase_key: 'plan' })],
        })}
      />,
    )

    expect(screen.getByText('統合')).toBeInTheDocument()
    expect(screen.getByText('claude-sonnet-4')).toBeInTheDocument()
    expect(screen.getByText('待機')).toBeInTheDocument()
  })

  it('item を持つステップ群は枝として表示する', () => {
    const branches = [
      makeStep({
        phase_key: 'investigate',
        item: { title: '円安の進行が主因' },
        item_label: '円安の進行が主因',
        status: 'completed',
      }),
      makeStep({
        phase_key: 'investigate',
        item: { title: '半導体サイクルの反転' },
        item_label: '半導体サイクルの反転',
        status: 'running',
        finished_at: undefined,
      }),
    ]

    render(<TaskExecutionTree {...makeProps({ steps: branches })} />)

    expect(
      screen.getByRole('button', { name: /円安の進行が主因/ }),
    ).toBeInTheDocument()
    expect(
      screen.getByRole('button', { name: /半導体サイクルの反転/ }),
    ).toBeInTheDocument()
    expect(screen.getAllByText('deepseek-v4-flash')).toHaveLength(2)
    expect(screen.getByText('完了')).toBeInTheDocument()
    expect(screen.getByText('実行中')).toBeInTheDocument()
  })

  it('output スキーマに enum 項目があれば、行のバッジをステータス文言の代わりに enum 値で表示する', () => {
    const configPhases = [
      {
        key: 'investigate',
        label: '仮説の調査',
        model: 'deepseek-v4-flash',
        output: { verdict: { enum: ['supported', 'rejected'] } },
      },
    ]
    const branches = [
      makeStep({
        phase_key: 'investigate',
        item: { title: '円安の進行が主因' },
        item_label: '円安の進行が主因',
        status: 'completed',
        output: { verdict: 'supported' },
      }),
    ]

    render(
      <TaskExecutionTree {...makeProps({ configPhases, steps: branches })} />,
    )

    expect(screen.getByText('supported')).toBeInTheDocument()
    expect(screen.queryByText('完了')).not.toBeInTheDocument()
  })

  it('output がまだ無いステップは enum バッジが無いのでステータス文言を表示する', () => {
    const configPhases = [
      {
        key: 'investigate',
        label: '仮説の調査',
        model: 'deepseek-v4-flash',
        output: { verdict: { enum: ['supported', 'rejected'] } },
      },
    ]
    const branches = [
      makeStep({
        phase_key: 'investigate',
        item: { title: '半導体サイクルの反転' },
        item_label: '半導体サイクルの反転',
        status: 'running',
        finished_at: undefined,
      }),
    ]

    render(
      <TaskExecutionTree {...makeProps({ configPhases, steps: branches })} />,
    )

    expect(screen.getByText('実行中')).toBeInTheDocument()
  })

  it('ノードをクリックすると item/output の詳細が開閉する', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
      item_label: '円安の進行が主因',
      output: { verdict: '妥当' },
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)
    const button = screen.getByRole('button', { name: /円安の進行が主因/ })

    expect(screen.queryByText('input')).not.toBeInTheDocument()
    expect(screen.queryByText('output')).not.toBeInTheDocument()

    await userEvent.click(button)
    expect(screen.getByText('input')).toBeInTheDocument()
    expect(
      screen.getByText('{ "title": "円安の進行が主因" }'),
    ).toBeInTheDocument()
    expect(screen.getByText('output')).toBeInTheDocument()
    expect(screen.getByText('{ "verdict": "妥当" }')).toBeInTheDocument()

    await userEvent.click(button)
    expect(screen.queryByText('input')).not.toBeInTheDocument()
    expect(screen.queryByText('output')).not.toBeInTheDocument()
  })

  it('traceUrlTemplate があれば選択中ステップのトレースリンクを組み立てる', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
      item_label: '円安の進行が主因',
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
      screen.getByRole('button', { name: /円安の進行が主因/ }),
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
      item: { title: '円安の進行が主因' },
      item_label: '円安の進行が主因',
      trace_id: 'trace-abc',
      span_id: 'span-def',
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)
    await userEvent.click(
      screen.getByRole('button', { name: /円安の進行が主因/ }),
    )

    expect(
      screen.queryByRole('link', { name: '→ トレースを開く' }),
    ).not.toBeInTheDocument()
  })

  it('failed ステータスのステップを選択すると error を表示する', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '半導体サイクルの反転' },
      item_label: '半導体サイクルの反転',
      status: 'failed',
      finished_at: '2026-08-15T00:00:05Z',
      error: 'tool call timeout',
    })

    render(<TaskExecutionTree {...makeProps({ steps: [step] })} />)
    await userEvent.click(
      screen.getByRole('button', { name: /半導体サイクルの反転/ }),
    )

    expect(screen.getByText('tool call timeout')).toBeInTheDocument()
  })

  it('detailPlacement が external のときクリックしてもインライン detail を出さない', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
      item_label: '円安の進行が主因',
      output: { verdict: '妥当' },
    })

    render(
      <TaskExecutionTree
        {...makeProps({ steps: [step], detailPlacement: 'external' })}
      />,
    )
    await userEvent.click(
      screen.getByRole('button', { name: /円安の進行が主因/ }),
    )

    expect(screen.queryByText('output')).not.toBeInTheDocument()
  })

  it('行の選択/選択解除のたびに onSelectStep を呼ぶ', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
      item_label: '円安の進行が主因',
    })
    const onSelectStep = vi.fn()

    render(
      <TaskExecutionTree {...makeProps({ steps: [step], onSelectStep })} />,
    )
    const button = screen.getByRole('button', { name: /円安の進行が主因/ })

    await userEvent.click(button)
    expect(onSelectStep).toHaveBeenLastCalledWith(step)

    await userEvent.click(button)
    expect(onSelectStep).toHaveBeenLastCalledWith(null)
  })

  it('選択中の step の内容が変わったら (同じ key のまま) onSelectStep が新しい内容で再度呼ばれる', async () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
      item_label: '円安の進行が主因',
      status: 'running',
      finished_at: undefined,
    })
    const onSelectStep = vi.fn()

    const { rerender } = render(
      <TaskExecutionTree {...makeProps({ steps: [step], onSelectStep })} />,
    )
    await userEvent.click(
      screen.getByRole('button', { name: /円安の進行が主因/ }),
    )
    expect(onSelectStep).toHaveBeenLastCalledWith(step)

    const updatedStep: TaskStep = {
      ...step,
      status: 'completed',
      finished_at: '2026-08-15T00:00:08Z',
      output: { verdict: '妥当' },
    }
    rerender(
      <TaskExecutionTree
        {...makeProps({ steps: [updatedStep], onSelectStep })}
      />,
    )

    expect(onSelectStep).toHaveBeenLastCalledWith(updatedStep)
  })
})

describe('StepDetail', () => {
  it('item/output を JSON で表示する', () => {
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
      output: { verdict: '妥当' },
    })

    render(<StepDetail strategyId="strategy-1" step={step} />)

    expect(screen.getByText('input')).toBeInTheDocument()
    expect(
      screen.getByText('{ "title": "円安の進行が主因" }'),
    ).toBeInTheDocument()
    expect(screen.getByText('output')).toBeInTheDocument()
    expect(screen.getByText('{ "verdict": "妥当" }')).toBeInTheDocument()
  })

  it('traceUrlTemplate があればトレースリンクを組み立てる', () => {
    const step = makeStep({
      phase_key: 'investigate',
      trace_id: 'trace-abc',
      span_id: 'span-def',
    })

    render(
      <StepDetail
        strategyId="strategy-1"
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
      output: { verdict: 'rejected', note_id: 'note-abc' },
    })

    renderInRouter(<StepDetail strategyId="strategy-1" step={step} />)

    expect(
      await screen.findByRole('link', { name: '→ ノートを開く' }),
    ).toHaveAttribute('href', '/strategies/strategy-1/notes/note-abc')
  })

  it('output に note_id が無ければノートへのリンクを出さない', () => {
    const step = makeStep({
      phase_key: 'investigate',
      output: { verdict: 'rejected' },
    })

    render(<StepDetail strategyId="strategy-1" step={step} />)

    expect(
      screen.queryByRole('link', { name: '→ ノートを開く' }),
    ).not.toBeInTheDocument()
  })

  it('output の note_id が空文字列ならノートへのリンクを出さない', () => {
    const step = makeStep({
      phase_key: 'investigate',
      output: { verdict: 'rejected', note_id: '' },
    })

    render(<StepDetail strategyId="strategy-1" step={step} />)

    expect(
      screen.queryByRole('link', { name: '→ ノートを開く' }),
    ).not.toBeInTheDocument()
  })
})

describe('findEnumBadge', () => {
  it('output スキーマの enum 項目に対応する値があればそれを返す', () => {
    const outputSchema = {
      verdict: { enum: ['supported', 'rejected', 'inconclusive'] },
      summary: { type: 'string' },
    }

    expect(findEnumBadge(outputSchema, { verdict: 'rejected' })).toBe(
      'rejected',
    )
  })

  it('output スキーマが無ければ null を返す', () => {
    expect(findEnumBadge(undefined, { verdict: 'rejected' })).toBeNull()
  })

  it('output に enum 項目の値がまだ無ければ null を返す', () => {
    const outputSchema = { verdict: { enum: ['supported', 'rejected'] } }

    expect(findEnumBadge(outputSchema, undefined)).toBeNull()
    expect(findEnumBadge(outputSchema, { other: 'x' })).toBeNull()
  })

  it('enum 宣言の無い項目しか無ければ null を返す', () => {
    const outputSchema = { summary: { type: 'string' } }

    expect(findEnumBadge(outputSchema, { summary: 'done' })).toBeNull()
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
      '    label: 調査計画',
      '    model: claude-opus-4',
      '    prompt: p',
      '  - key: investigate',
      '    label: 調査',
      '    model: deepseek-v4-flash',
      '    prompt: p',
      '    for_each: plan.items',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
      { key: 'investigate', label: '調査', model: 'deepseek-v4-flash' },
    ])
  })

  it('extracts output schema when present', () => {
    const yaml = [
      'phases:',
      '  - key: investigate',
      '    label: 調査',
      '    model: deepseek-v4-flash',
      '    output:',
      '      verdict:',
      '        enum: [supported, rejected]',
      '      summary:',
      '        type: string',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      {
        key: 'investigate',
        label: '調査',
        model: 'deepseek-v4-flash',
        output: {
          verdict: { enum: ['supported', 'rejected'] },
          summary: { type: 'string' },
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

  it('skips phase entries missing required string fields', () => {
    const yaml = [
      'phases:',
      '  - key: plan',
      '    label: 調査計画',
      '    model: claude-opus-4',
      '  - key: broken',
      '    label: 壊れた設定',
    ].join('\n')

    expect(parseAgentGraphPhases(yaml)).toEqual([
      { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
    ])
  })
})

describe('buildPhaseNodes', () => {
  it('returns empty array when steps is empty, even with configured phases', () => {
    const configPhases = [
      { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
    ]
    expect(buildPhaseNodes(configPhases, [])).toEqual([])
  })

  it('marks configured phases with no steps as pending', () => {
    const configPhases = [
      { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
      { key: 'merge', label: '統合', model: 'claude-sonnet-4' },
    ]
    const steps = [makeStep({ phase_key: 'plan' })]

    expect(buildPhaseNodes(configPhases, steps)).toEqual([
      { kind: 'single', key: 'plan', step: steps[0] },
      {
        kind: 'pending',
        key: 'merge',
        label: '統合',
        model: 'claude-sonnet-4',
      },
    ])
  })

  it('groups steps carrying an item into a branch node', () => {
    const configPhases = [
      { key: 'investigate', label: '調査', model: 'deepseek-v4-flash' },
    ]
    const branchA = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
      item_label: '円安の進行が主因',
      started_at: '2026-08-15T00:00:01Z',
    })
    const branchB = makeStep({
      phase_key: 'investigate',
      item: { title: '半導体サイクルの反転' },
      item_label: '半導体サイクルの反転',
      status: 'running',
      finished_at: undefined,
      started_at: '2026-08-15T00:00:00Z',
    })

    expect(buildPhaseNodes(configPhases, [branchA, branchB])).toEqual([
      { kind: 'branch', key: 'investigate', branches: [branchB, branchA] },
    ])
  })

  it('configPhases の output をノードの outputSchema として運ぶ', () => {
    const outputSchema = { verdict: { enum: ['supported', 'rejected'] } }
    const configPhases = [
      {
        key: 'investigate',
        label: '調査',
        model: 'deepseek-v4-flash',
        output: outputSchema,
      },
    ]
    const step = makeStep({
      phase_key: 'investigate',
      item: { title: '円安の進行が主因' },
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
