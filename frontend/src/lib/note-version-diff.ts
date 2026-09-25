import { diffLines } from 'diff'

export type NoteVersionDiffKind = 'unchanged' | 'changed' | 'added' | 'removed'

export interface NoteVersionDiffRow {
  key: string
  kind: NoteVersionDiffKind
  oldLine: number | null
  oldText: string | null
  newLine: number | null
  newText: string | null
}

interface ChangedBlock {
  added: Array<{ line: number; text: string }>
  removed: Array<{ line: number; text: string }>
}

function splitLines(value: string): string[] {
  const lines = value.match(/[^\n]*\n|[^\n]+$/g) ?? []
  return lines.map((line) => (line.endsWith('\n') ? line.slice(0, -1) : line))
}

export function buildNoteVersionDiff(
  previousBody: string | null,
  currentBody: string,
): NoteVersionDiffRow[] {
  const oldBody = previousBody ?? ''
  const parts = diffLines(oldBody, currentBody)
  const rows: NoteVersionDiffRow[] = []
  let changedBlock: ChangedBlock = { added: [], removed: [] }
  let keyIndex = 0

  const flushChangedBlock = (): void => {
    const rowCount = Math.max(
      changedBlock.added.length,
      changedBlock.removed.length,
    )
    for (let index = 0; index < rowCount; index += 1) {
      const removed = changedBlock.removed[index] ?? null
      const added = changedBlock.added[index] ?? null
      const kind =
        removed != null && added != null
          ? 'changed'
          : removed != null
            ? 'removed'
            : 'added'
      rows.push({
        key: `change-${String(keyIndex)}`,
        kind,
        oldLine: removed?.line ?? null,
        oldText: removed?.text ?? null,
        newLine: added?.line ?? null,
        newText: added?.text ?? null,
      })
      keyIndex += 1
    }
    changedBlock = { added: [], removed: [] }
  }

  let oldLine = 1
  let newLine = 1
  for (const part of parts) {
    const lines = splitLines(part.value)
    if (part.added) {
      for (const text of lines) {
        changedBlock.added.push({ line: newLine, text })
        newLine += 1
      }
      continue
    }
    if (part.removed) {
      for (const text of lines) {
        changedBlock.removed.push({ line: oldLine, text })
        oldLine += 1
      }
      continue
    }

    flushChangedBlock()
    for (const text of lines) {
      rows.push({
        key: `same-${String(keyIndex)}`,
        kind: 'unchanged',
        oldLine,
        oldText: text,
        newLine,
        newText: text,
      })
      oldLine += 1
      newLine += 1
      keyIndex += 1
    }
  }
  flushChangedBlock()

  const hasOldTrailingLine = previousBody?.endsWith('\n') ?? false
  const hasNewTrailingLine = currentBody.endsWith('\n')
  if (hasOldTrailingLine || hasNewTrailingLine) {
    const kind =
      hasOldTrailingLine && hasNewTrailingLine
        ? 'unchanged'
        : hasOldTrailingLine
          ? 'removed'
          : 'added'
    rows.push({
      key: `trailing-${String(keyIndex)}`,
      kind,
      oldLine: hasOldTrailingLine ? oldLine : null,
      oldText: hasOldTrailingLine ? '' : null,
      newLine: hasNewTrailingLine ? newLine : null,
      newText: hasNewTrailingLine ? '' : null,
    })
  }

  if (rows.length === 0) {
    rows.push({
      key: 'empty-body',
      kind: previousBody == null ? 'added' : 'unchanged',
      oldLine: previousBody == null ? null : 1,
      oldText: previousBody == null ? null : '',
      newLine: 1,
      newText: '',
    })
  }

  return rows
}
