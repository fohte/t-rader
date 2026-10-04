import { describe, expect, it } from 'vitest'

import { validateSkillName } from '#components/strategy-settings/skill-name'

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
      expected:
        'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます',
    },
    {
      name: 'leading-underscore',
      input: '_foo',
      expected:
        'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます',
    },
    {
      name: 'leading-uppercase',
      input: 'Foo',
      expected:
        'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます',
    },
    {
      name: 'mixed-case',
      input: 'snapShot',
      expected:
        'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます',
    },
    {
      name: 'contains-space',
      input: 'snap shot',
      expected:
        'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます',
    },
    {
      name: 'contains-dot',
      input: 'snap.shot',
      expected:
        'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます',
    },
    {
      name: 'contains-slash',
      input: 'snap/shot',
      expected:
        'skill 名は [a-z0-9] で始まり、英小文字 / 数字 / _ / - のみ使用できます',
    },
  ])('$name', ({ input, expected }) => {
    expect(validateSkillName(input)).toBe(expected)
  })
})
