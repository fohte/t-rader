import { mainConfig, typescriptConfig } from '@fohte/eslint-config'

const config = [
  { ignores: ['dist', 'vitest.config.ts'] },
  ...mainConfig,
  ...typescriptConfig,
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
]

export default config
