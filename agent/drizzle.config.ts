import { defineConfig } from 'drizzle-kit'

// `generate` only diffs the local schema against migration snapshots and
// never opens a connection, so it works fine against a placeholder URL.
const url =
  process.env['DATABASE_URL'] ??
  (process.argv.includes('generate')
    ? 'postgresql://localhost:5432/placeholder'
    : undefined)
if (url === undefined) {
  // eslint-disable-next-line no-restricted-syntax -- drizzle-kit の config 読み込み時の起動時検証、fail fast
  throw new Error(
    'DATABASE_URL is required (run `docker compose -f ../docker-compose.infra.yml port db 5432` for the local Postgres URL)',
  )
}

export default defineConfig({
  dialect: 'postgresql',
  schema: './src/db/schema.ts',
  out: './drizzle',
  dbCredentials: { url },
  strict: true,
  verbose: true,
})
