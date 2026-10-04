import { describe, expect, it } from 'vitest'

import { validateSkillName } from '#components/strategy-settings/skill-name'

const INVALID_SKILL_NAME_MESSAGE =
  'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます'

describe('validateSkillName', () => {
  it.each([
    { name: 'empty', input: '', expected: 'skill 名を入力してください' },
    { name: 'single-alpha', input: 'a', expected: null },
    { name: 'lowercase-word', input: 'snapshot', expected: null },
    { name: 'with-hyphen', input: 'check-pr-review', expected: null },
    { name: 'with-underscore', input: 'skill_01', expected: null },
    { name: 'leading-digit', input: '9to5', expected: null },
    {
      name: 'leading-hyphen',
      input: '-foo',
      expected: INVALID_SKILL_NAME_MESSAGE,
    },
    {
      name: 'leading-underscore',
      input: '_foo',
      expected: INVALID_SKILL_NAME_MESSAGE,
    },
    {
      name: 'leading-uppercase',
      input: 'Foo',
      expected: INVALID_SKILL_NAME_MESSAGE,
    },
    {
      name: 'mixed-case',
      input: 'snapShot',
      expected: INVALID_SKILL_NAME_MESSAGE,
    },
    {
      name: 'contains-space',
      input: 'snap shot',
      expected: INVALID_SKILL_NAME_MESSAGE,
    },
    {
      name: 'contains-dot',
      input: 'snap.shot',
      expected: INVALID_SKILL_NAME_MESSAGE,
    },
    {
      name: 'contains-slash',
      input: 'snap/shot',
      expected: INVALID_SKILL_NAME_MESSAGE,
    },
  ])('$name', ({ input, expected }) => {
    expect(validateSkillName(input)).toBe(expected)
  })
})
