import { describe, expect, it } from 'vitest'

import {
  SKILL_NAME_ERROR_EMPTY,
  SKILL_NAME_ERROR_INVALID,
  validateSkillName,
} from '@/components/strategy-settings/skill-name'

describe('validateSkillName', () => {
  it.each([
    { name: 'empty', input: '', expected: SKILL_NAME_ERROR_EMPTY },
    { name: 'single-alpha', input: 'a', expected: null },
    { name: 'lowercase-word', input: 'snapshot', expected: null },
    { name: 'with-hyphen', input: 'check-pr-review', expected: null },
    { name: 'with-underscore', input: 'skill_01', expected: null },
    { name: 'leading-digit', input: '9to5', expected: null },
    {
      name: 'leading-hyphen',
      input: '-foo',
      expected: SKILL_NAME_ERROR_INVALID,
    },
    {
      name: 'leading-underscore',
      input: '_foo',
      expected: SKILL_NAME_ERROR_INVALID,
    },
    {
      name: 'leading-uppercase',
      input: 'Foo',
      expected: SKILL_NAME_ERROR_INVALID,
    },
    {
      name: 'mixed-case',
      input: 'snapShot',
      expected: SKILL_NAME_ERROR_INVALID,
    },
    {
      name: 'contains-space',
      input: 'snap shot',
      expected: SKILL_NAME_ERROR_INVALID,
    },
    {
      name: 'contains-dot',
      input: 'snap.shot',
      expected: SKILL_NAME_ERROR_INVALID,
    },
    {
      name: 'contains-slash',
      input: 'snap/shot',
      expected: SKILL_NAME_ERROR_INVALID,
    },
  ])('$name', ({ input, expected }) => {
    expect(validateSkillName(input)).toBe(expected)
  })
})
