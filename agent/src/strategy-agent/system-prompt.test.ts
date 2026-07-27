import { describe, expect, it } from 'vitest'

import { buildSystemPrompt } from '#strategy-agent/system-prompt'

describe('buildSystemPrompt', () => {
  it('concatenates AGENTS.md with each skill under its own heading', () => {
    const actual = buildSystemPrompt({
      agentsMd: '# AGENTS\n\nbe a good analyst',
      skills: {
        'ja-stock': 'read Japanese stock filings',
        macro: 'track macro indicators',
      },
    })

    expect(actual).toBe(
      [
        '# AGENTS\n\nbe a good analyst',
        '# Skill: ja-stock\n\nread Japanese stock filings',
        '# Skill: macro\n\ntrack macro indicators',
      ].join('\n\n'),
    )
  })

  it('returns only the trimmed AGENTS.md when there are no skills', () => {
    expect(
      buildSystemPrompt({ agentsMd: '  be a good analyst  \n', skills: {} }),
    ).toBe('be a good analyst')
  })

  it('omits skills with empty or whitespace-only bodies', () => {
    const actual = buildSystemPrompt({
      agentsMd: 'AGENTS',
      skills: { empty: '   \n', real: 'has content' },
    })

    expect(actual).toBe('AGENTS\n\n# Skill: real\n\nhas content')
  })
})
