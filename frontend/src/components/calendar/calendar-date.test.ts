import { describe, expect, it } from 'vitest'

import {
  formatCalendarWeekRange,
  getCalendarWeekRange,
  getTokyoDate,
  shiftCalendarWeek,
  validateCalendarSearch,
} from '#components/calendar/calendar-date'

describe('getTokyoDate', () => {
  it('uses the date in Tokyo when UTC has not reached midnight there', () => {
    expect(getTokyoDate(new Date('2099-01-04T15:30:00Z'))).toBe('2099-01-05')
  })
})

describe('getCalendarWeekRange', () => {
  it.each([
    ['a Monday', '2099-01-05', { from: '2099-01-05', to: '2099-01-11' }],
    ['a Sunday', '2099-01-11', { from: '2099-01-05', to: '2099-01-11' }],
    ['the next Monday', '2099-01-12', { from: '2099-01-12', to: '2099-01-18' }],
  ])('returns the Monday to Sunday range for %s', (_name, date, expected) => {
    expect(getCalendarWeekRange(date)).toEqual(expected)
  })
})

describe('shiftCalendarWeek', () => {
  it.each([
    [-1, '2098-12-29'],
    [1, '2099-01-12'],
  ])('moves a week by %s', (amount, expected) => {
    expect(shiftCalendarWeek('2099-01-05', amount)).toBe(expected)
  })
})

describe('formatCalendarWeekRange', () => {
  it('shows both years when a week crosses into a new year', () => {
    expect(
      formatCalendarWeekRange({ from: '2098-12-28', to: '2099-01-03' }),
    ).toBe('2098/12/28 – 2099/1/3')
  })
})

describe('validateCalendarSearch', () => {
  it.each([
    [
      'valid filters',
      {
        strategy: '00000000-0000-0000-0000-000000000001',
        week: '2099-01-06',
        country: 'US',
      },
      {
        country: 'US',
        strategy: '00000000-0000-0000-0000-000000000001',
        week: '2099-01-05',
      },
    ],
    [
      'invalid filters',
      { strategy: '', week: '2099-02-30', country: 'ZZ' },
      {},
    ],
  ])('keeps only valid search values for %s', (_name, search, expected) => {
    expect(validateCalendarSearch(search)).toEqual(expected)
  })
})
