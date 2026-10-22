import { Button } from '@fohte/ui/button'
import { Checkbox } from '@fohte/ui/checkbox'
import { Input } from '@fohte/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'
import { Textarea } from '@fohte/ui/textarea'

import { PurposeSelect } from '#components/purpose-select'
import type { components } from '#lib/api/schema.gen'
import { parseJson } from '#lib/json'

type Trigger = components['schemas']['Trigger']
type TriggerKind = components['schemas']['TriggerKind']

const TRIGGER_KINDS: Array<{ label: string; value: TriggerKind }> = [
  { label: 'cron', value: 'cron' },
  { label: 'hook', value: 'hook' },
  { label: 'event', value: 'event' },
]

export interface FormState {
  purpose: string
  kind: TriggerKind
  schedule: string
  hookSlug: string
  eventType: string
  eventMatch: string
  promptTemplate: string
  enabled: boolean
}

export const EMPTY_FORM: FormState = {
  purpose: '',
  kind: 'cron',
  schedule: '',
  hookSlug: '',
  eventType: '',
  eventMatch: '',
  promptTemplate: '',
  enabled: true,
}

function parseKind(value: string): TriggerKind {
  if (value === 'hook' || value === 'event') return value
  return 'cron'
}

export function toFormState(trigger: Trigger): FormState {
  return {
    purpose: trigger.purpose ?? '',
    kind: parseKind(trigger.kind),
    schedule: trigger.schedule ?? '',
    hookSlug: trigger.hook_slug ?? '',
    eventType: trigger.event_type ?? '',
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
  } else if (form.kind === 'hook') {
    if (form.hookSlug.trim() === '') {
      return { ok: false, error: 'hook_slug は必須です' }
    }
  } else if (form.eventType.trim() === '') {
    return { ok: false, error: 'event_type は必須です' }
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
  defaultKindSelectOpen?: boolean
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
  defaultKindSelectOpen,
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

  const purposes = agentConfigs.map((config) => config.purpose)
  if (form.purpose !== '' && !purposes.includes(form.purpose)) {
    purposes.unshift(form.purpose)
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
        <PurposeSelect
          purposes={purposes}
          selectedPurpose={form.purpose}
          onPurposeChange={(purpose) => {
            update('purpose', purpose)
          }}
          disabled={isSaving}
        />
        <p className="font-mono text-2xs text-muted-foreground">
          未指定時は default の agent 設定を使用します。
        </p>
      </div>

      <div className="space-y-1.5">
        <span
          id="trigger-kind-label"
          className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          kind
        </span>
        <Select
          defaultOpen={defaultKindSelectOpen}
          items={TRIGGER_KINDS}
          value={form.kind}
          disabled={mode === 'edit'}
          onValueChange={(value) => {
            if (value != null) update('kind', parseKind(value))
          }}
        >
          <SelectTrigger
            id="trigger-kind"
            aria-labelledby="trigger-kind-label"
            className="w-full"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {TRIGGER_KINDS.map(({ label, value }) => (
              <SelectItem key={value} value={value}>
                {label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        {mode === 'edit' && (
          <p className="font-mono text-2xs text-muted-foreground">
            kind は作成後に変更できません。
          </p>
        )}
      </div>

      {form.kind === 'cron' && (
        <div className="space-y-1.5 font-mono">
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
      )}

      {form.kind === 'hook' && (
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

      {form.kind === 'event' && (
        <div className="space-y-1.5">
          <label
            className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
            htmlFor="trigger-event-type"
          >
            event_type
          </label>
          <Input
            id="trigger-event-type"
            value={form.eventType}
            placeholder="sample_process.finished"
            onChange={(e) => {
              update('eventType', e.target.value)
            }}
          />
        </div>
      )}

      {(form.kind === 'hook' || form.kind === 'event') && (
        <div className="space-y-1.5 font-mono">
          <label
            className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
            htmlFor="trigger-event-match"
          >
            event_match (JSON、空欄なら無条件)
          </label>
          <Textarea
            id="trigger-event-match"
            value={form.eventMatch}
            placeholder='{"event": {"eq": "fired"}}'
            onChange={(e) => {
              update('eventMatch', e.target.value)
            }}
            rows={4}
          />
        </div>
      )}

      <div className="space-y-1.5 font-mono">
        <label
          className="block font-mono text-2xs uppercase tracking-wider text-muted-foreground"
          htmlFor="trigger-prompt"
        >
          prompt_template
        </label>
        <Textarea
          id="trigger-prompt"
          value={form.promptTemplate}
          placeholder="morning briefing for {{strategy.name}}"
          onChange={(e) => {
            update('promptTemplate', e.target.value)
          }}
          rows={6}
        />
      </div>

      <div className="flex items-center gap-2 font-mono text-xs">
        <Checkbox
          aria-labelledby="trigger-enabled-label"
          checked={form.enabled}
          onCheckedChange={(checked) => {
            update('enabled', checked)
          }}
        />
        <span id="trigger-enabled-label">enabled</span>
      </div>

      {formError != null && (
        <p
          data-testid="trigger-form-error"
          className="font-mono text-2xs text-destructive"
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
