// Must run before any instrumented module is imported, otherwise
// @opentelemetry/auto-instrumentations-node cannot patch them. Either
// `import './bootstrap'` as the very first statement of the entrypoint,
// or pre-load with `node --import` (ESM) / `--require` (CJS).
import {
  initObservability,
  isObservabilityConfigured,
  type ObservabilityHandle,
} from '@fohte/service-kit/observability'

import { createJsonStdoutLogger } from '@/logger'

const jsonLogger = createJsonStdoutLogger()
const observabilityLogger = {
  info: (payload: Record<string, unknown>, msg: string) => {
    jsonLogger.log(msg, payload)
  },
  warn: (payload: Record<string, unknown>, msg: string) => {
    jsonLogger.log(msg, payload)
  },
}

export const initFromEnv = (
  env: Readonly<Record<string, string | undefined>> = process.env,
): ObservabilityHandle | undefined => {
  // Vitest sets NODE_ENV=test; skip initializing real Sentry/OTel
  // connections so test runs don't hang on open handles or ship telemetry.
  if (env['NODE_ENV'] === 'test') return undefined
  return isObservabilityConfigured(env)
    ? initObservability(env, { logger: observabilityLogger })
    : undefined
}

export const observability = initFromEnv()
