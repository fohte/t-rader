import postgres from 'postgres'
import { afterAll, afterEach, beforeEach, describe } from 'vitest'

const LOCAL_HOSTS = new Set(['127.0.0.1', 'localhost', '::1'])

export const TEST_DATABASE_URL = process.env['TEST_DATABASE_URL']

if (TEST_DATABASE_URL !== undefined) {
  const host = new URL(TEST_DATABASE_URL).hostname
  if (!LOCAL_HOSTS.has(host)) {
    // eslint-disable-next-line no-restricted-syntax -- TEST_DATABASE_URL の安全ガード、fail fast
    throw new Error(
      `TEST_DATABASE_URL must point at a local Postgres (got host: ${host}); ` +
        `the test setup runs DROP SCHEMA CASCADE`,
    )
  }
}

// `describe.skip` when no DB URL is configured so unit-only runs stay green
// without touching Postgres.
export const describeIfDb =
  TEST_DATABASE_URL === undefined ? describe.skip : describe

// One shared pool per vitest worker. Created lazily so a unit-only run never
// opens a connection.
let pool: postgres.Sql | null = null

const getPool = (): postgres.Sql => {
  if (pool === null) {
    if (TEST_DATABASE_URL === undefined) {
      // eslint-disable-next-line no-restricted-syntax -- describeIfDb でスキップされるはずの呼び出しが紛れ込んだ場合の fail fast
      throw new Error('TEST_DATABASE_URL is not set')
    }
    pool = postgres(TEST_DATABASE_URL, { max: 8, onnotice: () => {} })
  }
  return pool
}

// Close the pool once the worker is done so vitest can exit cleanly.
afterAll(async () => {
  if (pool !== null) {
    await pool.end({ timeout: 5 })
    pool = null
  }
})

// Read-only Sql for assertions against the migrated schema. Do not use this
// for tests that mutate state — they should take a tx from `setupTx` instead
// so the work is rolled back.
export const getTestSql = (): postgres.Sql => getPool()

// The returned getter throws if called outside a test — `reserved` is only
// set between this fixture's beforeEach and afterEach.
export const setupTx = (): (() => postgres.Sql) => {
  let reserved: postgres.ReservedSql | null = null

  beforeEach(async () => {
    reserved = await getPool().reserve()
    await reserved.unsafe('BEGIN')
  })

  afterEach(async () => {
    const r = reserved
    reserved = null
    if (r !== null) {
      // eslint-disable-next-line no-restricted-syntax -- release() を finally で必ず呼ぶため try/finally が必要
      try {
        await r.unsafe('ROLLBACK')
      } finally {
        r.release()
      }
    }
  })

  return () => {
    if (reserved === null) {
      // eslint-disable-next-line no-restricted-syntax -- テスト外からのアクセス防止、fail fast
      throw new Error('tx accessed outside a test (call setupTx in describe)')
    }
    return reserved
  }
}

// For code that builds a `drizzle(sql)` instance internally. drizzle-orm's
// postgres-js session reads `client.options.parsers`/`client.options.serializers`
// while constructing, but postgres-js's `sql.reserve()` rebuilds the
// tagged-template function from scratch and never copies `.options` onto it,
// so `drizzle(tx)` throws on a plain `setupTx()` result.
export const setupDrizzleTx = (): (() => postgres.Sql) => {
  const getTx = setupTx()

  return () => {
    const tx = getTx()
    const poolOptions = getPool().options
    Object.assign(tx, {
      options: {
        ...poolOptions,
        parsers: { ...poolOptions.parsers },
        serializers: { ...poolOptions.serializers },
      },
    })
    return tx
  }
}
