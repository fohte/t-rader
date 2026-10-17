import { describe, expect, it } from 'vitest'
import { parseDocument } from 'yaml'

import { parseWidgetDefinition } from '#lib/widget-definition'

const VALID_WIDGET_DEFINITION = [
  'queries:',
  '  daily:',
  '    metric: change',
  '    period: 1d',
  '    by: group',
  '    axis: demo-axis',
  '    agg: mean',
  'formulas:',
  '  doubled: daily * 2',
  'viz:',
  '  type: toplist',
  '  value: doubled',
  '  order: desc',
  '  limit: 5',
  'time:',
  '  as_of: 2099-01-05',
].join('\n')

describe('parseWidgetDefinition', () => {
  it('parses formulas into a widget query request', () => {
    expect(parseWidgetDefinition(VALID_WIDGET_DEFINITION)).toEqual({
      ok: true,
      request: {
        queries: {
          daily: {
            metric: 'change',
            period: '1d',
            by: 'group',
            axis: 'demo-axis',
            agg: 'mean',
          },
        },
        formulas: { doubled: 'daily * 2' },
        viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
        time: { as_of: '2099-01-05' },
      },
      viz: { type: 'toplist', value: 'doubled', order: 'desc', limit: 5 },
      asOf: '2099-01-05',
    })
  })

  it.each([
    {
      name: 'an impossible calendar date',
      source: VALID_WIDGET_DEFINITION.replace('2099-01-05', '2099-02-30'),
      message: 'queries、対応する viz、time.as_of を含む定義が必要です。',
    },
    {
      name: 'a non-string formula',
      source: VALID_WIDGET_DEFINITION.replace(
        'doubled: daily * 2',
        'doubled: 2',
      ),
      message: 'formulas.doubled は文字列にしてください。',
    },
    {
      name: 'a universe with both group and stocks',
      source: VALID_WIDGET_DEFINITION.replace(
        '    agg: mean',
        '    agg: mean\n    universe:\n      group: demo-group\n      stocks:\n        - demo-code',
      ),
      message: 'queries.daily の形式が正しくありません。',
    },
    {
      name: 'an unsupported visualization',
      source: VALID_WIDGET_DEFINITION.replace('type: toplist', 'type: table'),
      message: 'queries、対応する viz、time.as_of を含む定義が必要です。',
    },
    {
      name: 'a toplist limit outside the API range',
      source: VALID_WIDGET_DEFINITION.replace('limit: 5', 'limit: 5000000000'),
      message: 'queries、対応する viz、time.as_of を含む定義が必要です。',
    },
  ])('rejects $name', ({ source, message }) => {
    expect(parseWidgetDefinition(source)).toEqual({ ok: false, message })
  })

  it('reports YAML syntax errors', () => {
    const source = 'queries: ['
    const parserMessage =
      parseDocument(source, { uniqueKeys: true }).errors[0]?.message ??
      '解析エラーがありません。'

    expect(parseWidgetDefinition(source)).toEqual({
      ok: false,
      message: `YAML を解析できません: ${parserMessage}`,
    })
  })
})
