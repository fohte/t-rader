import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { InputBindingsField } from '#components/strategy-settings/agent-graph/fields/input-bindings-field'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)
vi.mock('#components/indicators/monaco-setup', () => ({}))

afterEach(cleanup)

describe('InputBindingsField', () => {
  it('YAML の入力 binding を構造として反映する', () => {
    const onChange = vi.fn()
    render(<InputBindingsField value={{}} onChange={onChange} />)

    fireEvent.change(screen.getByLabelText('入力 binding'), {
      target: {
        value:
          'request:\n  literal: { count: 3 }\nprevious:\n  prior_output:\n    phase: prepare\n    pointer: /items\n    limit: 25\nsource:\n  dataset:\n    provider: sample-provider\n    params: { code: DEMO }',
      },
    })

    expect(onChange).toHaveBeenLastCalledWith({
      request: { literal: { count: 3 } },
      previous: {
        prior_output: { phase: 'prepare', pointer: '/items', limit: 25 },
      },
      source: {
        dataset: { provider: 'sample-provider', params: { code: 'DEMO' } },
      },
    })
  })
})
