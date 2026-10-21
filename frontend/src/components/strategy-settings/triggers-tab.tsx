import { Button } from '@fohte/ui/button'
import { ButtonGroup } from '@fohte/ui/button-group'
import { Checkbox } from '@fohte/ui/checkbox'
import { Chip } from '@fohte/ui/chip'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'

import type { FormState } from '#components/strategy-settings/trigger-form'
import {
  EMPTY_FORM,
  toFormState,
  TriggerForm,
  validateForm,
} from '#components/strategy-settings/trigger-form'
import { Skeleton } from '#components/ui/skeleton'
import { $api } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type Trigger = components['schemas']['Trigger']

interface TriggersTabProps {
  strategyId: string
}

function triggerLabel(trigger: Trigger): string {
  switch (trigger.kind) {
    case 'cron':
      return trigger.schedule ?? ''
    case 'hook':
      return trigger.hook_slug ?? ''
    case 'event':
      return trigger.event_type ?? ''
    default:
      return ''
  }
}

export function TriggersTab({ strategyId }: TriggersTabProps) {
  const queryClient = useQueryClient()
  const listQueryOptions = $api.queryOptions(
    'get',
    '/api/strategies/{id}/triggers',
    { params: { path: { id: strategyId } } },
  )
  const { data, isPending, isError, error } = useQuery(listQueryOptions)
  const { data: agentConfigs = [] } = $api.useQuery('get', '/api/agent-configs')

  const triggers = useMemo(() => data ?? [], [data])

  const createMutation = $api.useMutation(
    'post',
    '/api/strategies/{id}/triggers',
  )
  const updateMutation = $api.useMutation('put', '/api/triggers/{trigger_id}')
  const deleteMutation = $api.useMutation(
    'delete',
    '/api/triggers/{trigger_id}',
  )

  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [mode, setMode] = useState<'view' | 'create'>('view')
  const [form, setForm] = useState<FormState>(EMPTY_FORM)
  const [formError, setFormError] = useState<string | null>(null)
  const [listError, setListError] = useState<string | null>(null)

  const selectedTrigger = useMemo(
    () => triggers.find((t) => t.trigger_id === selectedId) ?? null,
    [triggers, selectedId],
  )

  // form は「選択 trigger が切り替わった瞬間」だけ hydrate する。
  // 一覧 refetch (window focus / 他 trigger の toggle 操作後の invalidate 等) で
  // selectedTrigger の reference が変わるたびに setForm すると、編集中の入力が消える
  useEffect(() => {
    if (mode === 'create') return
    if (triggers.length === 0) {
      setSelectedId(null)
      setForm(EMPTY_FORM)
      return
    }
    const exists = triggers.some((t) => t.trigger_id === selectedId)
    if (selectedId != null && exists) return
    const first = triggers[0]
    if (first == null) return
    setSelectedId(first.trigger_id)
    setForm(toFormState(first))
  }, [mode, selectedId, triggers])

  function invalidate() {
    void queryClient.invalidateQueries({ queryKey: listQueryOptions.queryKey })
  }

  function startCreate() {
    setMode('create')
    setSelectedId(null)
    setForm(EMPTY_FORM)
    setFormError(null)
  }

  function cancelCreate() {
    setMode('view')
    setFormError(null)
    const next = triggers[0] ?? null
    if (next != null) {
      setSelectedId(next.trigger_id)
      setForm(toFormState(next))
    }
  }

  function selectTrigger(id: string) {
    setMode('view')
    setSelectedId(id)
    setFormError(null)
    const next = triggers.find((t) => t.trigger_id === id)
    if (next != null) {
      setForm(toFormState(next))
    }
  }

  function handleCreate() {
    const result = validateForm(form)
    if (!result.ok) {
      setFormError(result.error)
      return
    }
    createMutation.mutate(
      {
        params: { path: { id: strategyId } },
        body: {
          kind: form.kind,
          purpose: form.purpose === '' ? null : form.purpose,
          schedule: form.kind === 'cron' ? form.schedule.trim() : null,
          hook_slug: form.kind === 'hook' ? form.hookSlug.trim() : null,
          event_type: form.kind === 'event' ? form.eventType.trim() : null,
          event_match: result.eventMatch,
          prompt_template: form.promptTemplate.trim(),
          enabled: form.enabled,
        },
      },
      {
        onSuccess: (created) => {
          invalidate()
          setMode('view')
          setSelectedId(created.trigger_id)
          setForm(toFormState(created))
          setFormError(null)
        },
        onError: (err: unknown) => {
          setFormError(
            err instanceof Error
              ? `trigger 作成に失敗しました: ${err.message}`
              : 'trigger 作成に失敗しました',
          )
        },
      },
    )
  }

  function handleUpdate() {
    if (selectedTrigger == null) return
    const result = validateForm(form)
    if (!result.ok) {
      setFormError(result.error)
      return
    }
    const body: components['schemas']['UpdateTriggerRequest'] = {
      purpose: form.purpose === '' ? null : form.purpose,
      prompt_template: form.promptTemplate.trim(),
      enabled: form.enabled,
      event_match: result.eventMatch,
    }
    if (form.kind === 'cron') {
      body.schedule = form.schedule.trim()
    } else if (form.kind === 'hook') {
      body.hook_slug = form.hookSlug.trim()
    } else {
      body.event_type = form.eventType.trim()
    }
    updateMutation.mutate(
      {
        params: { path: { trigger_id: selectedTrigger.trigger_id } },
        body,
      },
      {
        onSuccess: () => {
          invalidate()
          setFormError(null)
        },
        onError: (err: unknown) => {
          setFormError(
            err instanceof Error
              ? `trigger 更新に失敗しました: ${err.message}`
              : 'trigger 更新に失敗しました',
          )
        },
      },
    )
  }

  function handleDelete(trigger: Trigger) {
    if (!window.confirm(`trigger を削除しますか?`)) return
    deleteMutation.mutate(
      { params: { path: { trigger_id: trigger.trigger_id } } },
      {
        onSuccess: () => {
          invalidate()
          setListError(null)
        },
        onError: () => {
          setListError('trigger 削除に失敗しました')
        },
      },
    )
  }

  function handleToggleEnabled(trigger: Trigger, nextEnabled: boolean) {
    updateMutation.mutate(
      {
        params: { path: { trigger_id: trigger.trigger_id } },
        body: { enabled: nextEnabled },
      },
      {
        onSuccess: () => {
          invalidate()
          setListError(null)
          // 選択中の trigger を toggle した場合、フォーム側の enabled も追従させる。
          // form は refetch で hydrate しないため、ここで反映しないと保存時に古い値が PUT される
          if (selectedId === trigger.trigger_id && mode !== 'create') {
            setForm((prev) => ({ ...prev, enabled: nextEnabled }))
          }
        },
        onError: () => {
          setListError('enabled 切替に失敗しました')
        },
      },
    )
  }

  if (isPending) {
    return <Skeleton className="h-80 w-full" />
  }

  if (isError) {
    return (
      <p
        data-testid="trigger-list-error"
        className="font-mono text-xs text-destructive"
      >
        trigger 一覧の取得に失敗しました
        {error instanceof Error ? `: ${error.message}` : ''}
      </p>
    )
  }

  const editing = mode === 'create' || selectedTrigger != null
  const isCreate = mode === 'create'

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-(--grid-cols-triggers-sidebar)">
      <aside className="space-y-3">
        <Button type="button" onClick={startCreate}>
          + 新しい trigger
        </Button>

        {listError != null && (
          <p
            data-testid="trigger-list-error"
            className="font-mono text-2xs text-destructive"
          >
            {listError}
          </p>
        )}

        <ul data-testid="trigger-list" className="border border-border">
          {triggers.length === 0 && (
            <li className="px-3 py-2 font-mono text-xs text-muted-foreground">
              trigger がまだありません
            </li>
          )}
          {triggers.map((t) => {
            const label = triggerLabel(t)
            return (
              <li
                key={t.trigger_id}
                className="border-b border-border px-1 py-1 font-mono text-xs last:border-b-0"
              >
                <ButtonGroup
                  className="w-full"
                  aria-label={`trigger ${t.kind} ${label} の操作`}
                >
                  <Button
                    type="button"
                    variant="ghost"
                    onClick={() => {
                      selectTrigger(t.trigger_id)
                    }}
                    aria-pressed={selectedId === t.trigger_id && !isCreate}
                    className="min-w-0 flex-1 justify-start"
                  >
                    <span className="shrink-0 uppercase">{t.kind}</span>
                    <span className="min-w-0 flex-1 truncate text-muted-foreground">
                      {label}
                    </span>
                    {selectedId === t.trigger_id && !isCreate && (
                      <Chip tone="strong">選択中</Chip>
                    )}
                  </Button>
                  <Checkbox
                    className="mx-1"
                    aria-label={`trigger ${t.kind} ${label} の有効化`}
                    title={t.enabled ? '有効' : '無効'}
                    checked={t.enabled}
                    onCheckedChange={(checked) => {
                      handleToggleEnabled(t, checked)
                    }}
                  />
                  <Button
                    type="button"
                    variant="plain"
                    onClick={() => {
                      handleDelete(t)
                    }}
                    aria-label={`trigger ${t.kind} ${label} を削除`}
                    className="ml-1"
                  >
                    削除
                  </Button>
                </ButtonGroup>
              </li>
            )
          })}
        </ul>
      </aside>

      <section>
        {!editing ? (
          <p className="font-mono text-xs text-muted-foreground">
            左の一覧から trigger を選択するか、新規追加してください。
          </p>
        ) : (
          <TriggerForm
            mode={isCreate ? 'create' : 'edit'}
            form={form}
            agentConfigs={agentConfigs}
            onChange={setForm}
            formError={formError}
            isSaving={
              isCreate ? createMutation.isPending : updateMutation.isPending
            }
            onSubmit={isCreate ? handleCreate : handleUpdate}
            onCancel={isCreate ? cancelCreate : null}
          />
        )}
      </section>
    </div>
  )
}
