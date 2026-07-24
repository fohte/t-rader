import { config } from '@fohte/eslint-config'

export default config(
  {
    typescript: { typeChecked: true },
    errorHandling: {
      interopBoundaryFiles: [
        'src/a2a/**/*.ts',
        'src/genai/**/*.ts',
        'src/internal-api/**/*.ts',
        'src/db/**/*.ts',
        'src/app.ts',
        'src/env.ts',
        'src/main.ts',
        'src/index.ts',
        'src/strategy-agent/strategy-agent.ts',
        'src/strategy-resolution/mgmt-mcp-client.ts',
        // vitest fixtures: transaction/schema setup for the test DB, not
        // application domain logic.
        'src/test/**/*.ts',
        'drizzle.config.ts',
      ],
    },
  },
  {
    rules: {
      'no-restricted-imports': [
        'error',
        {
          patterns: [
            {
              group: ['./*', '../*'],
              message:
                'Please use absolute imports instead of relative imports.',
            },
          ],
        },
      ],
    },
  },
)
