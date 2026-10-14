import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'

type ChangeHistory = components['schemas']['ChangeHistory']

export interface HistoryEventData {
  entry: ChangeHistory
  label: string
}

export interface GroupedNoteHistoryEvents {
  orderedVersions: NoteVersion[]
  eventsByVersionId: Map<string, HistoryEventData[]>
  unmatchedEvents: HistoryEventData[]
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function readDiff(entry: ChangeHistory): Record<string, unknown> | null {
  return isRecord(entry.diff_json) ? entry.diff_json : null
}

function readString(entry: ChangeHistory, key: string): string | null {
  const value = readDiff(entry)?.[key]
  return typeof value === 'string' ? value : null
}

function readStatusChangeVersionId(entry: ChangeHistory): string | null {
  return readString(entry, 'version_id')
}

function readSupersededVersionIds(entry: ChangeHistory): string[] {
  const ids = readDiff(entry)?.superseded_version_ids
  return Array.isArray(ids)
    ? ids.filter((id): id is string => typeof id === 'string')
    : []
}

function statusChangeLabel(entry: ChangeHistory): string {
  const target = readString(entry, 'to')
  if (target === 'approved') return '承認'
  if (target === 'rejected') return '却下'
  if (target === 'superseded') return '置き換え'
  return '状態変更'
}

function operationLabel(entry: ChangeHistory): string {
  if (entry.op === 'create') return '作成'
  if (entry.op === 'update') return '更新'
  if (entry.op === 'delete') return '削除'
  return '変更'
}

function latestVersionAt(
  orderedVersions: NoteVersion[],
  createdAt: string,
): NoteVersion | undefined {
  const eventTime = Date.parse(createdAt)
  if (Number.isNaN(eventTime)) return undefined

  return orderedVersions.reduce<NoteVersion | undefined>((latest, version) => {
    const versionTime = Date.parse(version.created_at)
    if (Number.isNaN(versionTime) || versionTime > eventTime) return latest

    const latestTime =
      latest == null ? Number.NEGATIVE_INFINITY : Date.parse(latest.created_at)
    return versionTime > latestTime ||
      (versionTime === latestTime &&
        (latest == null || version.version_no > latest.version_no))
      ? version
      : latest
  }, undefined)
}

function compareCreatedAtDescending(
  left: HistoryEventData,
  right: HistoryEventData,
): number {
  return Date.parse(right.entry.created_at) - Date.parse(left.entry.created_at)
}

export function groupNoteHistoryEvents(
  versions: NoteVersion[],
  history: ChangeHistory[],
): GroupedNoteHistoryEvents {
  const orderedVersions = [...versions].sort(
    (left, right) => right.version_no - left.version_no,
  )
  const versionsById = new Map(
    orderedVersions.map((version) => [version.id, version]),
  )
  const latestVersionNo = orderedVersions[0]?.version_no
  const eventsByVersionId = new Map<string, HistoryEventData[]>()
  const unmatchedEvents: HistoryEventData[] = []

  const addEvent = (versionId: string | null, event: HistoryEventData) => {
    const targetVersion =
      (versionId == null ? undefined : versionsById.get(versionId)) ??
      latestVersionAt(orderedVersions, event.entry.created_at)
    if (targetVersion == null) {
      unmatchedEvents.push(event)
      return
    }
    const versionEvents = eventsByVersionId.get(targetVersion.id) ?? []
    versionEvents.push(event)
    eventsByVersionId.set(targetVersion.id, versionEvents)
  }

  for (const entry of history) {
    if (entry.op === 'status_change') {
      const event = { entry, label: statusChangeLabel(entry) }
      const versionId = readStatusChangeVersionId(entry)
      addEvent(versionId, event)
      for (const supersededVersionId of readSupersededVersionIds(entry)) {
        if (supersededVersionId !== versionId) {
          addEvent(supersededVersionId, { entry, label: '置き換え' })
        }
      }
      continue
    }

    const toVersionId = readString(entry, 'to_version_id')
    const targetVersion =
      toVersionId == null ? undefined : versionsById.get(toVersionId)
    const isCurrentVersionChange =
      entry.op === 'update' &&
      targetVersion != null &&
      latestVersionNo != null &&
      targetVersion.version_no < latestVersionNo
    addEvent(toVersionId, {
      entry,
      label: isCurrentVersionChange ? '現行に変更' : operationLabel(entry),
    })
  }

  for (const events of eventsByVersionId.values()) {
    events.sort(compareCreatedAtDescending)
  }
  unmatchedEvents.sort(compareCreatedAtDescending)

  return { orderedVersions, eventsByVersionId, unmatchedEvents }
}
