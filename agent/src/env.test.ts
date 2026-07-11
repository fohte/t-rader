import { describe, expect, it } from 'vitest'

import { EnvError, loadEnv } from '@/env'

const fullSource = {
  DATABASE_URL: 'postgres://localhost/t_rader_agent',
  TRADER_AGENT_PORT: '8080',
  TRADER_AGENT_URL: 'http://t-rader-agent:8080/',
  INTERNAL_API_TOKEN: 'internal-token',
  BACKEND_WEBHOOK_URL: 'http://backend/api/agent-tasks/notifications',
  BACKEND_WEBHOOK_TOKEN: 'webhook-token',
  A2A_WATCHDOG_TIMEOUT_MS: '60000',
  A2A_RETENTION_DAYS: '7',
} as const

const captureIssues = (run: () => unknown): readonly string[] => {
  try {
    run()
  } catch (err) {
    if (err instanceof EnvError) return err.issues
    throw err
  }
  throw new Error('expected loadEnv to throw')
}

describe('loadEnv', () => {
  it('parses a complete environment', () => {
    expect(loadEnv(fullSource)).toEqual({
      DATABASE_URL: 'postgres://localhost/t_rader_agent',
      TRADER_AGENT_PORT: 8080,
      TRADER_AGENT_URL: 'http://t-rader-agent:8080/',
      INTERNAL_API_TOKEN: 'internal-token',
      BACKEND_WEBHOOK_URL: 'http://backend/api/agent-tasks/notifications',
      BACKEND_WEBHOOK_TOKEN: 'webhook-token',
      A2A_WATCHDOG_TIMEOUT_MS: 60000,
      A2A_RETENTION_DAYS: 7,
    })
  })

  it('defaults A2A_WATCHDOG_TIMEOUT_MS and A2A_RETENTION_DAYS when omitted', () => {
    const {
      A2A_WATCHDOG_TIMEOUT_MS: _timeout,
      A2A_RETENTION_DAYS: _retention,
      ...rest
    } = fullSource
    void _timeout
    void _retention
    const env = loadEnv(rest)
    expect({
      watchdogTimeoutMs: env.A2A_WATCHDOG_TIMEOUT_MS,
      retentionDays: env.A2A_RETENTION_DAYS,
    }).toEqual({
      watchdogTimeoutMs: 10 * 60 * 1000,
      retentionDays: 30,
    })
  })

  it('fails fast listing every missing required key', () => {
    expect(captureIssues(() => loadEnv({}))).toEqual([
      'missing required env: DATABASE_URL',
      'TRADER_AGENT_PORT must be a positive integer (got: undefined)',
      'missing required env: TRADER_AGENT_URL',
      'missing required env: INTERNAL_API_TOKEN',
      'missing required env: BACKEND_WEBHOOK_URL',
      'missing required env: BACKEND_WEBHOOK_TOKEN',
    ])
  })

  it('rejects a non-positive-integer TRADER_AGENT_PORT', () => {
    expect(
      captureIssues(() => loadEnv({ ...fullSource, TRADER_AGENT_PORT: '0' })),
    ).toEqual(['TRADER_AGENT_PORT must be a positive integer (got: 0)'])
  })

  it('rejects a non-positive-integer A2A_WATCHDOG_TIMEOUT_MS', () => {
    expect(
      captureIssues(() =>
        loadEnv({ ...fullSource, A2A_WATCHDOG_TIMEOUT_MS: '-1' }),
      ),
    ).toEqual(['A2A_WATCHDOG_TIMEOUT_MS must be a positive integer (got: -1)'])
  })
})
