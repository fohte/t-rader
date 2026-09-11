import { createFileRoute, Link } from '@tanstack/react-router'
import { useMemo, useState } from 'react'

import { FilterBar, type FilterOption } from '#components/runs/filter-bar'
import { TaskRunListView } from '#components/strategy-shell/task-run-list-view'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/runs')({
  component: RunsPage,
})

// purpose 未指定 (purpose カラム追加前に作成された行) を絞り込むための内部キー
const UNSPECIFIED_PURPOSE = '__unspecified__'

function RunsPage() {
  const [strategyFilter, setStrategyFilter] = useState('all')
  const [purposeFilter, setPurposeFilter] = useState('all')

  const { data: tasks, isPending } = $api.useQuery('get', '/api/tasks')
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: agentConfigs = [] } = $api.useQuery('get', '/api/agent-configs')

  const taskList = useMemo(() => tasks ?? [], [tasks])

  const strategyNameById = useMemo(
    () => new Map(strategies.map((s) => [s.id, s.name] as const)),
    [strategies],
  )

  const strategyOptions = useMemo<FilterOption[]>(() => {
    const counts = new Map<string, number>()
    for (const t of taskList) {
      counts.set(t.strategy_id, (counts.get(t.strategy_id) ?? 0) + 1)
    }
    return strategies.map((s) => ({
      id: s.id,
      label: s.name,
      count: counts.get(s.id) ?? 0,
    }))
  }, [taskList, strategies])

  const purposeOptions = useMemo<FilterOption[]>(() => {
    const counts = new Map<string, number>()
    for (const t of taskList) {
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
  }, [taskList, agentConfigs])

  const shown = useMemo(
    () =>
      taskList
        .filter(
          (t) => strategyFilter === 'all' || t.strategy_id === strategyFilter,
        )
        .filter((t) => {
          if (purposeFilter === 'all') return true
          if (purposeFilter === UNSPECIFIED_PURPOSE) return t.purpose == null
          return t.purpose === purposeFilter
        }),
    [taskList, strategyFilter, purposeFilter],
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
        allCount={taskList.length}
      />
      <FilterBar
        options={purposeOptions}
        value={purposeFilter}
        onChange={setPurposeFilter}
        allLabel="すべての目的"
        allCount={taskList.length}
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
