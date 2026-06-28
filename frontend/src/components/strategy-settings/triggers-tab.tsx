import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useEffect, useMemo, useState } from 'react'

import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Skeleton } from '@/components/ui/skeleton'
import { $api } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type Trigger = components['schemas']['Trigger']
type TriggerKind = components['schemas']['TriggerKind']

interface TriggersTabProps {
  strategyId: string
}

interface FormState {
  kind: TriggerKind
  schedule: string
  hookSlug: string
  eventMatch: string
  promptTemplate: string
  enabled: boolean
}

const EMPTY_FORM: FormState = {
  kind: 'cron',
  schedule: '',
  hookSlug: '',
  eventMatch: '',
  promptTemplate: '',
  enabled: true,
}

function parseKind(value: string): TriggerKind {
  return value === 'hook' ? 'hook' : 'cron'
}

function triggerLabel(trigger: Trigger): string {
  return trigger.kind === 'cron'
    ? (trigger.schedule ?? '')
    : (trigger.hook_slug ?? '')
}

function toFormState(trigger: Trigger): FormState {
  return {
    kind: parseKind(trigger.kind),
    schedule: trigger.schedule ?? '',
    hookSlug: trigger.hook_slug ?? '',
    eventMatch:
      trigger.event_match != null
        ? JSON.stringify(trigger.event_match, null, 2)
        : '',
    promptTemplate: trigger.prompt_template,
    enabled: trigger.enabled,
  }
}

interface ValidationResult {
  ok: boolean
  error: string | null
  eventMatch?: Record<string, unknown> | null
}

function validateForm(form: FormState): ValidationResult {
  if (form.promptTemplate.trim() === '') {
    return { ok: false, error: 'prompt_template は必須です' }
  }
  if (form.kind === 'cron') {
    if (form.schedule.trim() === '') {
      return { ok: false, error: 'schedule (cron 式) は必須です' }
    }
  } else if (form.hookSlug.trim() === '') {
    return { ok: false, error: 'hook_slug は必須です' }
  }
  let eventMatch: Record<string, unknown> | null = null
  const trimmed = form.eventMatch.trim()
  if (trimmed !== '') {
    let parsed: unknown
    try {
      parsed = JSON.parse(trimmed)
    } catch {
      return { ok: false, error: 'event_match の JSON が不正です' }
    }
    if (parsed == null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return {
        ok: false,
        error: 'event_match は JSON object である必要があります',
      }
    }
    eventMatch = { ...parsed }
  }
  return { ok: true, error: null, eventMatch }
}

export function TriggersTab({ strategyId }: TriggersTabProps) {
  const queryClient = useQueryClient()
  const listQueryOptions = $api.queryOptions(
    'get',
    '/api/strategies/{id}/triggers',
    { params: { path: { id: strategyId } } },
  )
  const { data, isPending, isError, error } = useQuery(listQueryOptions)

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
    if (selectedId != null) return
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
          schedule: form.kind === 'cron' ? form.schedule.trim() : null,
          hook_slug: form.kind === 'hook' ? form.hookSlug.trim() : null,
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
      prompt_template: form.promptTemplate.trim(),
      enabled: form.enabled,
      event_match: result.eventMatch,
    }
    if (form.kind === 'cron') {
      body.schedule = form.schedule.trim()
    } else {
      body.hook_slug = form.hookSlug.trim()
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
          if (selectedId === trigger.trigger_id) {
            setSelectedId(null)
            setForm(EMPTY_FORM)
          }
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
    return <Skeleton className="h-[320px] w-full" />
  }

  if (isError) {
    return (
      <p
        data-testid="trigger-list-error"
        className="font-mono text-[12px] text-[color:var(--color-accent-strategy)]"
      >
        trigger 一覧の取得に失敗しました
        {error instanceof Error ? `: ${error.message}` : ''}
      </p>
    )
  }

  const editing = mode === 'create' || selectedTrigger != null
  const isCreate = mode === 'create'

  return (
    <div className="grid grid-cols-1 gap-4 lg:grid-cols-[280px_minmax(0,1fr)]">
      <aside className="space-y-3">
        <Button type="button" onClick={startCreate}>
          + 新しい trigger
        </Button>

        {listError != null && (
          <p
            data-testid="trigger-list-error"
            className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
          >
            {listError}
          </p>
        )}

        <ul
          data-testid="trigger-list"
          className="border border-[color:var(--color-hairline)]"
        >
          {triggers.length === 0 && (
            <li className="px-3 py-2 font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
              trigger がまだありません
            </li>
          )}
          {triggers.map((t) => {
            const label = triggerLabel(t)
            return (
              <li
                key={t.trigger_id}
                className="flex items-center gap-1 border-b border-[color:var(--color-hairline)] px-1 last:border-b-0"
              >
                <button
                  type="button"
                  onClick={() => {
                    selectTrigger(t.trigger_id)
                  }}
                  data-active={selectedId === t.trigger_id && !isCreate}
                  className="flex-1 truncate px-2 py-1.5 text-left font-mono text-[12px] hover:bg-[color:var(--panel-inset)] data-[active=true]:text-[color:var(--color-accent-strategy)]"
                >
                  <span className="uppercase">{t.kind}</span>
                  <span className="ml-2 text-[color:var(--color-text-tertiary)]">
                    {label}
                  </span>
                </button>
                <label
                  className="flex items-center px-1 font-mono text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
                  title={t.enabled ? '有効' : '無効'}
                >
                  <input
                    type="checkbox"
                    aria-label={`trigger ${t.kind} ${label} の有効化`}
                    checked={t.enabled}
                    onChange={(e) => {
                      handleToggleEnabled(t, e.target.checked)
                    }}
                  />
                </label>
                <button
                  type="button"
                  onClick={() => {
                    handleDelete(t)
                  }}
                  aria-label={`trigger ${t.kind} ${label} を削除`}
                  className="px-2 py-1 font-mono text-[11px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-accent-strategy)]"
                >
                  削除
                </button>
              </li>
            )
          })}
        </ul>
      </aside>

      <section>
        {!editing ? (
          <p className="font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
            左の一覧から trigger を選択するか、新規追加してください。
          </p>
        ) : (
          <TriggerForm
            mode={isCreate ? 'create' : 'edit'}
            form={form}
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

interface TriggerFormProps {
  mode: 'create' | 'edit'
  form: FormState
  onChange: (next: FormState) => void
  formError: string | null
  isSaving: boolean
  onSubmit: () => void
  onCancel: (() => void) | null
}

function TriggerForm({
  mode,
  form,
  onChange,
  formError,
  isSaving,
  onSubmit,
  onCancel,
}: TriggerFormProps) {
  function update<K extends keyof FormState>(key: K, value: FormState[K]) {
    onChange({ ...form, [key]: value })
  }

  return (
    <form
      data-testid="trigger-form"
      className="space-y-3"
      onSubmit={(e) => {
        e.preventDefault()
        onSubmit()
      }}
    >
      <div className="space-y-1.5">
        <label
          className="block font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
          htmlFor="trigger-kind"
        >
          kind
        </label>
        <select
          id="trigger-kind"
          value={form.kind}
          disabled={mode === 'edit'}
          onChange={(e) => {
            update('kind', parseKind(e.target.value))
          }}
          className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-[12px]"
        >
          <option value="cron">cron</option>
          <option value="hook">hook</option>
        </select>
        {mode === 'edit' && (
          <p className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
            kind は作成後に変更できません。
          </p>
        )}
      </div>

      {form.kind === 'cron' ? (
        <div className="space-y-1.5">
          <label
            className="block font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
            htmlFor="trigger-schedule"
          >
            schedule (cron 式 UTC)
          </label>
          <Input
            id="trigger-schedule"
            value={form.schedule}
            placeholder="0 9 * * 1-5"
            onChange={(e) => {
              update('schedule', e.target.value)
            }}
          />
        </div>
      ) : (
        <div className="space-y-1.5">
          <label
            className="block font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
            htmlFor="trigger-hook-slug"
          >
            hook_slug (POST /api/hooks/:slug)
          </label>
          <Input
            id="trigger-hook-slug"
            value={form.hookSlug}
            placeholder="tv-alert"
            onChange={(e) => {
              update('hookSlug', e.target.value)
            }}
          />
        </div>
      )}

      {form.kind === 'hook' && (
        <div className="space-y-1.5">
          <label
            className="block font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
            htmlFor="trigger-event-match"
          >
            event_match (JSON、空欄なら無条件)
          </label>
          <textarea
            id="trigger-event-match"
            value={form.eventMatch}
            placeholder='{"event": {"eq": "fired"}}'
            onChange={(e) => {
              update('eventMatch', e.target.value)
            }}
            rows={4}
            className="w-full resize-y border border-input bg-transparent p-2 font-mono text-[12px]"
          />
        </div>
      )}

      <div className="space-y-1.5">
        <label
          className="block font-mono text-[11px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]"
          htmlFor="trigger-prompt"
        >
          prompt_template
        </label>
        <textarea
          id="trigger-prompt"
          value={form.promptTemplate}
          placeholder="morning briefing for {{strategy.name}}"
          onChange={(e) => {
            update('promptTemplate', e.target.value)
          }}
          rows={6}
          className="w-full resize-y border border-input bg-transparent p-2 font-mono text-[12px]"
        />
      </div>

      <label
        className="flex items-center gap-2 font-mono text-[12px]"
        htmlFor="trigger-enabled"
      >
        <input
          id="trigger-enabled"
          type="checkbox"
          checked={form.enabled}
          onChange={(e) => {
            update('enabled', e.target.checked)
          }}
        />
        enabled
      </label>

      {formError != null && (
        <p
          data-testid="trigger-form-error"
          className="font-mono text-[11px] text-[color:var(--color-accent-strategy)]"
        >
          {formError}
        </p>
      )}

      <div className="flex items-center gap-3">
        <Button type="submit" disabled={isSaving}>
          {isSaving ? '保存中…' : mode === 'create' ? '作成' : '保存'}
        </Button>
        {onCancel != null && (
          <Button type="button" variant="outline" onClick={onCancel}>
            キャンセル
          </Button>
        )}
      </div>
    </form>
  )
}
