const dateTimeFormatter = new Intl.DateTimeFormat('ja-JP', {
  dateStyle: 'short',
  timeStyle: 'short',
  timeZone: 'Asia/Tokyo',
})

export function formatIngestDateTime(value: string | null | undefined): string {
  if (value == null) return '—'

  const date = new Date(value)
  return Number.isNaN(date.getTime()) ? value : dateTimeFormatter.format(date)
}

export function formatIngestDate(value: string | null | undefined): string {
  return value ?? '—'
}

export function formatIngestStats(stats: unknown): string {
  if (stats == null) return '—'
  if (typeof stats !== 'object' || Array.isArray(stats)) {
    return formatIngestStatValue(stats)
  }

  const entries = Object.entries(stats)
  if (entries.length === 0) return '—'

  return entries
    .map(([key, value]) => `${key}: ${formatIngestStatValue(value)}`)
    .join(', ')
}

function formatIngestStatValue(value: unknown): string {
  if (value == null) return '—'
  if (typeof value === 'number') return value.toLocaleString('ja-JP')
  if (typeof value === 'string' || typeof value === 'boolean') {
    return String(value)
  }
  return JSON.stringify(value)
}
