import { TabsPanel } from '@fohte/ui/tabs'
import { createFileRoute, redirect, useNavigate } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { FilterBar, type FilterOption } from '#components/runs/filter-bar'
import { RunsTabHeader } from '#components/runs/runs-tab-header'
import { CreateStrategyCard } from '#components/strategy-home/create-strategy-card'
import { CreateStrategyDialog } from '#components/strategy-home/create-strategy-dialog'
import { StrategyCard } from '#components/strategy-home/strategy-card'
import { StrategyListCount } from '#components/strategy-home/strategy-list-count'
import { StrategyListIntroduction } from '#components/strategy-home/strategy-list-introduction'
import { StrategyListLoading } from '#components/strategy-home/strategy-list-loading'
import {
  StrategyListTabs,
  validateStrategyListSearch,
} from '#components/strategy-home/strategy-list-tabs'
import { TaskRunListView } from '#components/strategy-shell/task-run-list-view'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/strategies/')({
  validateSearch: validateStrategyListSearch,
  beforeLoad: ({ search }) => {
    if (search.legacyIndicatorsTab === true) {
      // eslint-disable-next-line @typescript-eslint/only-throw-error, no-restricted-syntax -- TanStack Router の redirect API は throw を要求する
      throw redirect({ to: '/settings/indicators' })
    }
  },
  component: StrategyListPage,
})

const UNSPECIFIED_PURPOSE = '__unspecified__'

function StrategyListPage() {
  const { strategyTab: requestedTab } = Route.useSearch()
  const tab = requestedTab ?? 'list'
  const navigate = useNavigate({ from: '/strategies/' })

  return (
    <div className="font-sans text-foreground">
      <StrategyListTabs
        activeTab={tab}
        onChange={(nextTab) => {
          void navigate({
            to: '/strategies',
            search: { strategyTab: nextTab },
          })
        }}
      >
        <TabsPanel value="list">
          {tab === 'list' && <StrategyListContent />}
        </TabsPanel>
        <TabsPanel value="runs">{tab === 'runs' && <RunsTab />}</TabsPanel>
      </StrategyListTabs>
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
      <StrategyListIntroduction />

      {isPending ? (
        <StrategyListLoading />
      ) : (
        <>
          <StrategyListCount count={strategies?.length ?? 0} />
          <div className="grid grid-cols-1 gap-3.5 sm:grid-cols-2 lg:grid-cols-3">
            {(strategies ?? []).map((strategy) => (
              <StrategyCard key={strategy.id} strategy={strategy} />
            ))}
            <CreateStrategyCard
              onClick={() => {
                setCreating(true)
              }}
            />
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
                modelCallLimitReached: task.model_call_limit_reached,
              }))
        }
      />
    </div>
  )
}
