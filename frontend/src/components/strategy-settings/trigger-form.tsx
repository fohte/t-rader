import { Button } from '@fohte/ui/button'
import { Input } from '@fohte/ui/input'

import type { components } from '#lib/api/schema.gen'
import { parseJson } from '#lib/json'

type Trigger = components['schemas']['Trigger']
type TriggerKind = components['schemas']['TriggerKind']

export interface FormState {
  purpose: string
  kind: TriggerKind
  schedule: string
  hookSlug: string
  eventMatch: string
  promptTemplate: string
  enabled: boolean
}

export const EMPTY_FORM: FormState = {
  purpose: '',
  kind: 'cron',
  schedule: '',
  hookSlug: '',
  eventMatch: '',
  promptTemplate: '',
  enabled: true,
}

export function parseKind(value: string): TriggerKind {
  return value === 'hook' ? 'hook' : 'cron'
}

export function toFormState(trigger: Trigger): FormState {
  return {
    purpose: trigger.purpose ?? '',
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

export interface ValidationResult {
  ok: boolean
  error: string | null
  eventMatch?: Record<string, unknown> | null
}

export function validateForm(form: FormState): ValidationResult {
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
    const parsedResult = parseJson(trimmed)
    if (parsedResult.isErr()) {
      return { ok: false, error: 'event_match の JSON が不正です' }
    }
    const parsed = parsedResult.value
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

export interface TriggerFormProps {
  mode: 'create' | 'edit'
  form: FormState
  agentConfigs: ReadonlyArray<{ purpose: string }>
  onChange: (next: FormState) => void
  formError: string | null
  isSaving: boolean
  onSubmit: () => void
  onCancel: (() => void) | null
}

export function TriggerForm({
  mode,
  form,
  agentConfigs,
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
          className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
          htmlFor="trigger-purpose"
        >
          purpose
        </label>
        <select
          id="trigger-purpose"
          value={form.purpose}
          onChange={(e) => {
            update('purpose', e.target.value)
          }}
          className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-xs"
        >
          <option value="">default (未指定)</option>
          {form.purpose !== '' &&
            !agentConfigs.some((config) => config.purpose === form.purpose) && (
              <option value={form.purpose}>{form.purpose}</option>
            )}
          {agentConfigs.map((config) => (
            <option key={config.purpose} value={config.purpose}>
              {config.purpose}
            </option>
          ))}
        </select>
        <p className="font-mono text-2xs text-muted-foreground">
          未指定時は default の agent 設定を使用します。
        </p>
      </div>

      <div className="space-y-1.5">
        <label
          className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
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
          className="h-9 w-full rounded-md border border-input bg-transparent px-3 font-mono text-xs disabled:cursor-not-allowed disabled:opacity-50"
        >
          <option value="cron">cron</option>
          <option value="hook">hook</option>
        </select>
        {mode === 'edit' && (
          <p className="font-mono text-2xs text-muted-foreground">
            kind は作成後に変更できません。
          </p>
        )}
      </div>

      {form.kind === 'cron' ? (
        <div className="space-y-1.5">
          <label
            className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
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
            className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
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
            className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
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
            className="w-full resize-y border border-input bg-transparent p-2 font-mono text-xs"
          />
        </div>
      )}

      <div className="space-y-1.5">
        <label
          className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
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
          className="w-full resize-y border border-input bg-transparent p-2 font-mono text-xs"
        />
      </div>

      <label
        className="flex items-center gap-2 font-mono text-xs"
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
          className="font-mono text-2xs text-primary"
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
