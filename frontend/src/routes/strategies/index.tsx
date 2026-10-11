import { createFileRoute } from '@tanstack/react-router'
import { Plus } from 'lucide-react'
import { useMemo, useState } from 'react'

import { IndicatorsPage } from '#components/indicators/indicators-page'
import { FilterBar, type FilterOption } from '#components/runs/filter-bar'
import { RunsTabHeader } from '#components/runs/runs-tab-header'
import { CreateStrategyDialog } from '#components/strategy-home/create-strategy-dialog'
import { StrategyCard } from '#components/strategy-home/strategy-card'
import {
  parseStrategyListTab,
  type StrategyListTab,
  StrategyListTabs,
} from '#components/strategy-home/strategy-list-tabs'
import { TaskRunListView } from '#components/strategy-shell/task-run-list-view'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/strategies/')({
  validateSearch: (
    search: Record<string, unknown>,
  ): { strategyTab?: StrategyListTab } => {
    const strategyTab = parseStrategyListTab(search.strategyTab)
    return strategyTab ? { strategyTab } : {}
  },
  component: StrategyListPage,
})

const UNSPECIFIED_PURPOSE = '__unspecified__'

function StrategyListPage() {
  const { strategyTab: requestedTab } = Route.useSearch()
  const tab = requestedTab ?? 'list'
  return (
    <div className="font-sans text-foreground">
      <StrategyListTabs activeTab={tab} />
      {tab === 'list' ? (
        <StrategyListContent />
      ) : tab === 'runs' ? (
        <RunsTab />
      ) : (
        <IndicatorsPage scope="global" />
      )}
    </div>
  )
}

function StrategyListContent() {
  const [creating, setCreating] = useState(false)
  const { data: strategies, isPending } = $api.useQuery(
    'get',
    '/api/strategies',
  )
  return (
    <div className="font-sans text-foreground">
      <div className="mb-8 max-w-180">
        <h1 className="mb-3 text-2xl font-bold tracking-tight">
          <span className="font-mono font-bold text-primary">&gt;</span>{' '}
          戦略を選ぶ
        </h1>
        <p className="text-sm leading-relaxed text-muted-foreground-strong">
          各戦略は運用資金の区分です。取引の帰属先・投資可能額・リスク上限を管理します。分析は口座全体を対象に行います。
        </p>
      </div>

      {isPending ? (
        <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
          <Skeleton className="h-47" />
          <Skeleton className="h-47" />
          <Skeleton className="h-47" />
        </div>
      ) : (
        <>
          <div className="mb-2.5 flex items-baseline gap-2 font-mono text-2xs uppercase tracking-wider text-muted-foreground">
            <span className="text-primary">&gt;</span>
            <span>strategies</span>
            <span className="text-muted-foreground-strong">
              {strategies?.length ?? 0} 件
            </span>
          </div>
          <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
            {(strategies ?? []).map((strategy) => (
              <StrategyCard key={strategy.id} strategy={strategy} />
            ))}
            <button
              type="button"
              onClick={() => {
                setCreating(true)
              }}
              className="flex min-h-47 cursor-pointer flex-col items-center justify-center gap-2 border border-dashed border-border bg-transparent p-4 text-center text-muted-foreground hover:border-primary hover:text-primary"
            >
              <Plus className="size-6" />
              <div className="font-mono text-sm">新しい戦略を作る</div>
            </button>
          </div>
        </>
      )}

      <CreateStrategyDialog open={creating} onOpenChange={setCreating} />
    </div>
  )
}

function RunsTab() {
  const [strategyFilter, setStrategyFilter] = useState('all')
  const [purposeFilter, setPurposeFilter] = useState('all')

  // フィルタ選択肢の件数が切り替えで変動しないよう、常に全件クエリを見る。
  // /api/tasks は直近 50 件までのため、件数は正確な総数ではない。
  const { data: allTasks = [] } = $api.useQuery('get', '/api/tasks')
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: agentConfigs = [] } = $api.useQuery('get', '/api/agent-configs')

  const { data: filteredTasksRaw, isPending } = $api.useQuery(
    'get',
    '/api/tasks',
    {
      params: {
        query: {
          strategy_id: strategyFilter !== 'all' ? strategyFilter : undefined,
          purpose:
            purposeFilter !== 'all' && purposeFilter !== UNSPECIFIED_PURPOSE
              ? purposeFilter
              : undefined,
        },
      },
    },
  )

  const strategyNameById = useMemo(
    () =>
      new Map(
        strategies.map((strategy) => [strategy.id, strategy.name] as const),
      ),
    [strategies],
  )

  const strategyOptions = useMemo<FilterOption[]>(() => {
    const counts = new Map<string, number>()
    for (const task of allTasks) {
      counts.set(task.strategy_id, (counts.get(task.strategy_id) ?? 0) + 1)
    }
    return strategies.map((strategy) => ({
      id: strategy.id,
      label: strategy.name,
      count: counts.get(strategy.id) ?? 0,
    }))
  }, [allTasks, strategies])

  const purposeOptions = useMemo<FilterOption[]>(() => {
    const counts = new Map<string, number>()
    for (const task of allTasks) {
      const purpose = task.purpose ?? UNSPECIFIED_PURPOSE
      counts.set(purpose, (counts.get(purpose) ?? 0) + 1)
    }
    const options = agentConfigs.map((config) => ({
      id: config.purpose,
      label: config.purpose,
      count: counts.get(config.purpose) ?? 0,
    }))
    const unspecifiedCount = counts.get(UNSPECIFIED_PURPOSE) ?? 0
    if (unspecifiedCount > 0) {
      options.push({
        id: UNSPECIFIED_PURPOSE,
        label: '未設定',
        count: unspecifiedCount,
      })
    }
    return options
  }, [allTasks, agentConfigs])

  const shown = useMemo(
    () =>
      purposeFilter === UNSPECIFIED_PURPOSE
        ? (filteredTasksRaw ?? []).filter((task) => task.purpose == null)
        : (filteredTasksRaw ?? []),
    [filteredTasksRaw, purposeFilter],
  )

  return (
    <div className="space-y-5 font-sans text-foreground">
      <RunsTabHeader />

      <FilterBar
        options={strategyOptions}
        value={strategyFilter}
        onChange={setStrategyFilter}
        allLabel="すべての戦略"
        allCount={allTasks.length}
      />
      <FilterBar
        options={purposeOptions}
        value={purposeFilter}
        onChange={setPurposeFilter}
        allLabel="すべての目的"
        allCount={allTasks.length}
      />

      <TaskRunListView
        tasks={
          isPending
            ? null
            : shown.map((task) => ({
                taskId: task.task_id,
                strategyId: task.strategy_id,
                strategyName: strategyNameById.get(task.strategy_id),
                prompt: task.prompt,
                source: task.source,
                phase: task.phase,
                purpose: task.purpose ?? null,
                createdAt: task.created_at,
              }))
        }
      />
    </div>
  )
}
