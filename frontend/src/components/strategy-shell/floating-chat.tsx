import { useQueryClient } from '@tanstack/react-query'
import { useEffect, useState } from 'react'

import {
  closeFloatingChat,
  consumeFloatingChatSeed,
  openFloatingChat,
  useFloatingChat,
} from '@/components/strategy-shell/floating-chat-store'
import {
  type FloatingChatNote,
  type FloatingChatStatus,
  FloatingChatView,
} from '@/components/strategy-shell/floating-chat-view'
import { useCurrentStrategyId } from '@/components/strategy-shell/use-current-strategy-id'
import { $api } from '@/lib/api/client'

const POLL_INTERVAL_MS = 2000
// ノート紐付けは現状 created_at の比較で代替している。client 時刻が backend に
// 対して進んでいるとタスク経由で作成されたノートが取りこぼされるため、
// NTP 程度の skew を吸収できる猶予を入れる。
const SUBMITTED_AT_SKEW_MS = 60_000

interface CurrentTask {
  taskId: string
  submittedAt: string
}

export function FloatingChat(): React.ReactElement {
  const { open, seed: storeSeed } = useFloatingChat()
  const strategyId = useCurrentStrategyId() ?? null
  const queryClient = useQueryClient()

  const [seed, setSeed] = useState<string | null>(null)
  const [input, setInput] = useState('')
  const [currentTask, setCurrentTask] = useState<CurrentTask | null>(null)
  const [submitError, setSubmitError] = useState<string | null>(null)

  useEffect(() => {
    if (!open || storeSeed == null) return
    const s = consumeFloatingChatSeed()
    setSeed(s)
    setInput(s ?? '')
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

  const taskQuery = $api.useQuery(
    'get',
    '/api/strategies/{id}/tasks/{task_id}',
    {
      params: {
        path: {
          id: strategyId ?? '',
          task_id: currentTask?.taskId ?? '',
        },
      },
    },
    {
      enabled: strategyId != null && currentTask != null,
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

  // 完了直後にノート一覧が古いまま残ると新規ノートが見えないため、
  // 完了タイミングでキャッシュを破棄して再取得を促す。
  useEffect(() => {
    if (!isCompleted || strategyId == null) return
    void queryClient.invalidateQueries({
      queryKey: $api.queryOptions('get', '/api/notes', {
        params: { query: { strategy_id: strategyId } },
      }).queryKey,
    })
  }, [isCompleted, strategyId, queryClient])

  const notesQuery = $api.useQuery(
    'get',
    '/api/notes',
    {
      params: { query: { strategy_id: strategyId ?? '' } },
    },
    {
      enabled: isCompleted && strategyId != null,
    },
  )

  const generatedNotes: FloatingChatNote[] = (() => {
    if (!isCompleted || currentTask == null) return []
    const cutoff = new Date(
      Date.parse(currentTask.submittedAt) - SUBMITTED_AT_SKEW_MS,
    ).toISOString()
    return (notesQuery.data ?? [])
      .filter((n) => n.created_at >= cutoff)
      .sort((a, b) => b.created_at.localeCompare(a.created_at))
      .map((n) => ({ id: n.id, title: n.title, updated_at: n.updated_at }))
  })()

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
    setCurrentTask(null)
    submitMutation.mutate(
      { params: { path: { id: strategyId } }, body: { prompt } },
      {
        onSuccess: (data) => {
          setCurrentTask({
            taskId: data.task_id,
            submittedAt: new Date().toISOString(),
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
      seed={seed}
      input={input}
      status={status}
      notes={generatedNotes}
      onOpen={() => {
        openFloatingChat()
      }}
      onClose={() => {
        closeFloatingChat()
      }}
      onInputChange={setInput}
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
