import { createFileRoute, Link } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { FilterBar, type FilterOption } from '#components/runs/filter-bar'
import { TaskRunListView } from '#components/strategy-shell/task-run-list-view'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/runs')({
  component: RunsPage,
})

const UNSPECIFIED_PURPOSE = '__unspecified__'

function RunsPage() {
  const [strategyFilter, setStrategyFilter] = useState('all')
  const [purposeFilter, setPurposeFilter] = useState('all')

  // フィルタ選択肢 (件数バッジ) 計算用。フィルタ切り替えで件数が変動しないよう、常に全件クエリを見る。
  // ただし /api/tasks 自体にページネーションが無く直近 50 件までしか返らないため、この件数も正確な総数ではない。
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
    () => new Map(strategies.map((s) => [s.id, s.name] as const)),
    [strategies],
  )

  const strategyOptions = useMemo<FilterOption[]>(() => {
    const counts = new Map<string, number>()
    for (const t of allTasks) {
      counts.set(t.strategy_id, (counts.get(t.strategy_id) ?? 0) + 1)
    }
    return strategies.map((s) => ({
      id: s.id,
      label: s.name,
      count: counts.get(s.id) ?? 0,
    }))
  }, [allTasks, strategies])

  const purposeOptions = useMemo<FilterOption[]>(() => {
    const counts = new Map<string, number>()
    for (const t of allTasks) {
      const key = t.purpose ?? UNSPECIFIED_PURPOSE
      counts.set(key, (counts.get(key) ?? 0) + 1)
    }
    const options = agentConfigs.map((c) => ({
      id: c.purpose,
      label: c.purpose,
      count: counts.get(c.purpose) ?? 0,
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
        ? (filteredTasksRaw ?? []).filter((t) => t.purpose == null)
        : (filteredTasksRaw ?? []),
    [filteredTasksRaw, purposeFilter],
  )

  return (
    <div className="space-y-5 font-sans text-foreground">
      <div>
        <Link
          to="/strategies"
          className="font-mono text-xs text-muted-foreground hover:text-foreground"
        >
          &lt; 戦略一覧に戻る
        </Link>
      </div>

      <header>
        <h1 className="mb-1.5 text-2xl font-bold leading-tight tracking-tight">
          実行履歴
        </h1>
        <p className="max-w-180 text-sm leading-relaxed text-muted-foreground-strong">
          全戦略横断のエージェント実行履歴。戦略・目的で絞り込めます。
        </p>
      </header>

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
            : shown.map((t) => ({
                taskId: t.task_id,
                strategyId: t.strategy_id,
                strategyName: strategyNameById.get(t.strategy_id),
                prompt: t.prompt,
                source: t.source,
                phase: t.phase,
                purpose: t.purpose ?? null,
                createdAt: t.created_at,
              }))
        }
      />
    </div>
  )
}
