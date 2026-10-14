import { useQueryClient } from '@tanstack/react-query'
import { useLocation } from '@tanstack/react-router'
import { useEffect, useMemo, useState } from 'react'

import {
  closeFloatingChat,
  consumeFloatingChatSeed,
  openFloatingChat,
  useFloatingChat,
} from '#components/strategy-shell/floating-chat-store'
import {
  readLastUsedStrategyId,
  resolveDefaultStrategyId,
  saveLastUsedStrategyId,
} from '#components/strategy-shell/floating-chat-strategy'
import {
  type FloatingChatNote,
  type FloatingChatStatus,
  FloatingChatView,
} from '#components/strategy-shell/floating-chat-view'
import { useCurrentStrategyId } from '#components/strategy-shell/use-current-strategy-id'
import { $api } from '#lib/api/client'

const POLL_INTERVAL_MS = 2000

interface CurrentTask {
  strategyId: string
  taskId: string
}

interface StrategySelectionOverride {
  routePath: string
  selectedStrategyId: string | null
}

export function FloatingChat(): React.ReactElement {
  const { open, seed: storeSeed } = useFloatingChat()
  const routeStrategyId = useCurrentStrategyId() ?? null
  const routePath = useLocation({ select: (location) => location.pathname })
  const queryClient = useQueryClient()

  const [seed, setSeed] = useState<string | null>(null)
  const [input, setInput] = useState('')
  const [selectedPurpose, setSelectedPurpose] = useState('')
  const [submittedPrompt, setSubmittedPrompt] = useState<string | null>(null)
  const [lastUsedStrategyId, setLastUsedStrategyId] = useState(
    readLastUsedStrategyId,
  )
  const [strategySelectionOverride, setStrategySelectionOverride] =
    useState<StrategySelectionOverride | null>(null)
  const [currentTask, setCurrentTask] = useState<CurrentTask | null>(null)
  const [submitError, setSubmitError] = useState<string | null>(null)

  useEffect(() => {
    if (!open || storeSeed == null) return
    const s = consumeFloatingChatSeed()
    setSeed(s)
    setInput(s ?? '')
    setSubmittedPrompt(null)
    setCurrentTask(null)
    setSubmitError(null)
  }, [open, storeSeed])

  useEffect(() => {
    if (!open) return
    const handleKeyDown = (e: KeyboardEvent): void => {
      if (e.key === 'Escape') closeFloatingChat()
    }
    window.addEventListener('keydown', handleKeyDown)
    return () => {
      window.removeEventListener('keydown', handleKeyDown)
    }
  }, [open])

  const submitMutation = $api.useMutation('post', '/api/strategies/{id}/chat')
  const { data: strategies = [] } = $api.useQuery('get', '/api/strategies')
  const { data: agentConfigs = [] } = $api.useQuery('get', '/api/agent-configs')

  const hasSelectionOverride =
    strategySelectionOverride?.routePath === routePath
  const strategyId = hasSelectionOverride
    ? strategySelectionOverride.selectedStrategyId
    : resolveDefaultStrategyId({
        currentStrategyId: routeStrategyId,
        lastUsedStrategyId,
        availableStrategyIds: strategies.map((strategy) => strategy.id),
      })

  const taskQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}',
    {
      params: {
        path: {
          id: currentTask?.strategyId ?? '',
          task_id: currentTask?.taskId ?? '',
        },
      },
    },
    {
      enabled: currentTask != null,
      refetchInterval: (query) => {
        if (query.state.error != null) return false
        const data = query.state.data
        if (data == null) return POLL_INTERVAL_MS
        return data.phase === 'pending' || data.phase === 'running'
          ? POLL_INTERVAL_MS
          : false
      },
    },
  )

  const phase = taskQuery.data?.phase ?? null
  const isCompleted = phase === 'completed'

  useEffect(() => {
    if (!isCompleted) return
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes').queryKey,
    })
  }, [isCompleted, queryClient])

  const taskNotesQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}/notes',
    {
      params: {
        path: {
          id: currentTask?.strategyId ?? '',
          task_id: currentTask?.taskId ?? '',
        },
      },
    },
    {
      enabled: isCompleted && currentTask != null,
    },
  )

  const generatedNotes = useMemo<FloatingChatNote[]>(() => {
    if (!isCompleted || currentTask == null) return []
    return (taskNotesQuery.data ?? []).map((n) => ({
      id: n.id,
      title: n.title,
      updated_at: n.updated_at,
    }))
  }, [isCompleted, currentTask, taskNotesQuery.data])

  const status = computeStatus({
    submitting: submitMutation.isPending,
    submitError,
    taskError: taskQuery.error,
    hasCurrentTask: currentTask != null,
    phase,
    errorSummary: taskQuery.data?.error_summary ?? null,
  })

  function handleSubmit(): void {
    if (strategyId == null) return
    const prompt = input.trim()
    if (prompt === '') return
    setSubmitError(null)
    setSubmittedPrompt(prompt)
    setSeed(null)
    setCurrentTask(null)
    saveLastUsedStrategyId(strategyId)
    setLastUsedStrategyId(strategyId)
    submitMutation.mutate(
      {
        params: { path: { id: strategyId } },
        body: {
          prompt,
          ...(selectedPurpose === '' ? {} : { purpose: selectedPurpose }),
        },
      },
      {
        onSuccess: (data) => {
          setCurrentTask({
            strategyId,
            taskId: data.task_id,
          })
          setInput('')
        },
        onError: (err) => {
          setSubmitError(formatSubmitError(err))
        },
      },
    )
  }

  return (
    <FloatingChatView
      open={open}
      strategyId={strategyId}
      taskStrategyId={currentTask?.strategyId ?? null}
      strategies={strategies.map(({ id, name }) => ({ id, name }))}
      submittedPrompt={submittedPrompt}
      seed={seed}
      input={input}
      resultText={taskQuery.data?.result_text ?? null}
      status={status}
      notes={generatedNotes}
      purposes={agentConfigs.map((config) => config.purpose)}
      selectedPurpose={selectedPurpose}
      currentTaskId={currentTask?.taskId ?? null}
      onOpen={() => {
        openFloatingChat()
      }}
      onClose={() => {
        closeFloatingChat()
      }}
      onInputChange={setInput}
      onStrategyChange={(selectedStrategyId) => {
        setStrategySelectionOverride({
          routePath,
          selectedStrategyId,
        })
      }}
      onPurposeChange={setSelectedPurpose}
      onSubmit={handleSubmit}
    />
  )
}

function computeStatus({
  submitting,
  submitError,
  taskError,
  hasCurrentTask,
  phase,
  errorSummary,
}: {
  submitting: boolean
  submitError: string | null
  taskError: unknown
  hasCurrentTask: boolean
  phase: string | null
  errorSummary: string | null
}): FloatingChatStatus {
  if (submitting) return { kind: 'submitting' }
  if (submitError != null) return { kind: 'error', message: submitError }
  if (taskError != null) {
    return { kind: 'error', message: 'タスクの状態取得に失敗しました' }
  }
  if (phase === 'pending' || phase === 'running') {
    return { kind: 'polling', phase }
  }
  if (phase === 'completed') return { kind: 'completed' }
  if (phase === 'failed') {
    return { kind: 'failed', error_summary: errorSummary }
  }
  // submit 成功直後で初回 polling 結果が未着のとき。phase 未取得を idle に
  // 見せると一瞬「未投入」表示にチラつくため pending として継続表示する。
  if (hasCurrentTask) return { kind: 'polling', phase: 'pending' }
  return { kind: 'idle' }
}

function formatSubmitError(err: unknown): string {
  // backend は `{ error: "..." }` 形式 (ErrorResponse スキーマ) で返す。
  if (err != null && typeof err === 'object' && 'error' in err) {
    const m = (err as { error?: unknown }).error
    if (typeof m === 'string' && m !== '') return m
  }
  return 'タスクの投入に失敗しました'
}
