import { describe, expect, it } from 'vitest'

import {
  formatIngestDate,
  formatIngestDateTime,
  formatIngestStats,
} from '#lib/format-ingest-status'

describe('formatIngestDateTime', () => {
  it('formats timestamps in Japan Standard Time', () => {
    expect(formatIngestDateTime('2099-01-02T03:04:00Z')).toBe(
      '2099/01/02 12:04',
    )
  })

  it('keeps an invalid timestamp visible', () => {
    expect(formatIngestDateTime('invalid timestamp')).toBe('invalid timestamp')
  })

  it('shows a dash when the timestamp is missing', () => {
    expect(formatIngestDateTime(null)).toBe('—')
  })
})

describe('formatIngestDate', () => {
  it('shows an available date as supplied by the API', () => {
    expect(formatIngestDate('2099-01-02')).toBe('2099-01-02')
  })

  it('shows a dash when the date is missing', () => {
    expect(formatIngestDate(null)).toBe('—')
  })
})

describe('formatIngestStats', () => {
  it('formats each statistic as a key and value pair', () => {
    expect(formatIngestStats({ rows_written: 1234, skipped: 2 })).toBe(
      'rows_written: 1,234, skipped: 2',
    )
  })

  it.each([
    { name: 'missing', stats: null },
    { name: 'empty', stats: {} },
  ])('shows a dash for $name statistics', ({ stats }) => {
    expect(formatIngestStats(stats)).toBe('—')
  })
})
