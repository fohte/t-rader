export interface Env {
  DATABASE_URL: string
  TRADER_AGENT_PORT: number
  // Base URL this service is reachable at, published as the A2A Agent
  // Card's `url` field for future agent-to-agent callers.
  TRADER_AGENT_URL: string
  INTERNAL_API_TOKEN: string
  BACKEND_WEBHOOK_TOKEN: string
  A2A_WATCHDOG_TIMEOUT_MS: number
  A2A_RETENTION_DAYS: number
  LLM_CALL_TIMEOUT_MS: number
  // agent が backend API、webhook、MCP endpoint に接続する base URL。
  BACKEND_API_BASE_URL: string
  LLM_API_KEY: string
  LLM_BASE_URL: string | undefined
}

const DEFAULT_WATCHDOG_TIMEOUT_MS = 10 * 60 * 1000
const DEFAULT_RETENTION_DAYS = 30
const DEFAULT_LLM_CALL_TIMEOUT_MS = 5 * 60 * 1000

export class EnvError extends Error {
  constructor(
    public readonly issues: readonly string[],
    message?: string,
  ) {
    super(message ?? `invalid environment: ${issues.join('; ')}`)
    this.name = 'EnvError'
  }
}

export const loadEnv = (
  source: Readonly<Record<string, string | undefined>> = process.env,
): Env => {
  const issues: string[] = []

  const requireString = (key: keyof Env): string => {
    const raw = source[key]
    if (raw === undefined || raw === '') {
      issues.push(`missing required env: ${key}`)
      return ''
    }
    return raw
  }

  const requirePositiveInt = (key: keyof Env): number => {
    const raw = source[key]
    const parsed = Number(raw)
    if (
      raw === undefined ||
      raw === '' ||
      !Number.isInteger(parsed) ||
      parsed <= 0
    ) {
      issues.push(
        `${key} must be a positive integer (got: ${raw ?? 'undefined'})`,
      )
      return 0
    }
    return parsed
  }

  const parsePositiveIntWithDefault = (
    key: keyof Env,
    defaultValue: number,
  ): number => {
    const raw = source[key]
    if (raw === undefined) return defaultValue
    const parsed = Number(raw)
    if (raw === '' || !Number.isInteger(parsed) || parsed <= 0) {
      issues.push(`${key} must be a positive integer (got: ${raw})`)
      return defaultValue
    }
    return parsed
  }

  const optionalString = (key: keyof Env): string | undefined => {
    const raw = source[key]
    return raw === undefined || raw === '' ? undefined : raw
  }

  const env: Env = {
    DATABASE_URL: requireString('DATABASE_URL'),
    TRADER_AGENT_PORT: requirePositiveInt('TRADER_AGENT_PORT'),
    TRADER_AGENT_URL: requireString('TRADER_AGENT_URL'),
    INTERNAL_API_TOKEN: requireString('INTERNAL_API_TOKEN'),
    BACKEND_WEBHOOK_TOKEN: requireString('BACKEND_WEBHOOK_TOKEN'),
    A2A_WATCHDOG_TIMEOUT_MS: parsePositiveIntWithDefault(
      'A2A_WATCHDOG_TIMEOUT_MS',
      DEFAULT_WATCHDOG_TIMEOUT_MS,
    ),
    A2A_RETENTION_DAYS: parsePositiveIntWithDefault(
      'A2A_RETENTION_DAYS',
      DEFAULT_RETENTION_DAYS,
    ),
    LLM_CALL_TIMEOUT_MS: parsePositiveIntWithDefault(
      'LLM_CALL_TIMEOUT_MS',
      DEFAULT_LLM_CALL_TIMEOUT_MS,
    ),
    BACKEND_API_BASE_URL: requireString('BACKEND_API_BASE_URL'),
    LLM_API_KEY: requireString('LLM_API_KEY'),
    LLM_BASE_URL: optionalString('LLM_BASE_URL'),
  }

  if (issues.length > 0) {
    // eslint-disable-next-line no-restricted-syntax -- 起動時の env 検証、fail fast
    throw new EnvError(issues)
  }

  return env
}
