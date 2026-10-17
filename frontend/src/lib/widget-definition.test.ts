import { describe, expect, it } from 'vitest'
import { parseDocument } from 'yaml'

import {
  parseWidgetDefinition,
  parseWidgetQueryRequest,
} from '#lib/widget-definition'

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

const VALID_WIDGET_VALUE = {
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
}

const VALID_WIDGET_VALUE_WITHOUT_TIME = {
  queries: VALID_WIDGET_VALUE.queries,
  formulas: VALID_WIDGET_VALUE.formulas,
  viz: VALID_WIDGET_VALUE.viz,
}

const VALID_WIDGET_VALUE_WITHOUT_AS_OF = {
  ...VALID_WIDGET_VALUE,
  time: {},
}

const VALID_RESULT = {
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
}

describe('parseWidgetDefinition', () => {
  it('parses YAML into a widget query request', () => {
    expect(parseWidgetDefinition(VALID_WIDGET_DEFINITION)).toEqual(VALID_RESULT)
  })

  it('converts an already parsed widget mapping into a query request', () => {
    expect(parseWidgetQueryRequest(VALID_WIDGET_VALUE)).toEqual(VALID_RESULT)
  })

  it.each([
    {
      name: 'time',
      value: VALID_WIDGET_VALUE_WITHOUT_TIME,
      request: VALID_WIDGET_VALUE_WITHOUT_TIME,
    },
    {
      name: 'time.as_of',
      value: VALID_WIDGET_VALUE_WITHOUT_AS_OF,
      request: VALID_WIDGET_VALUE_WITHOUT_AS_OF,
    },
  ])('allows omitted $name', ({ value, request }) => {
    expect(parseWidgetQueryRequest(value)).toEqual({
      ok: true,
      request,
      viz: VALID_WIDGET_VALUE.viz,
      asOf: undefined,
    })
  })

  it('passes semantic validation values through for the API to validate', () => {
    const source = VALID_WIDGET_DEFINITION.replace(
      'metric: change',
      'metric: future-metric',
    )
      .replace('period: 1d', 'period: future-period')
      .replace('by: group', 'by: future-dimension')
      .replace('agg: mean', 'agg: future-aggregation')
      .replace('type: toplist', 'type: future-viz')
      .replace('order: desc', 'order: sideways')
      .replace('limit: 5', 'limit: -1')
      .replace('as_of: 2099-01-05', 'as_of: not-a-date')

    expect(parseWidgetDefinition(source)).toEqual({
      ok: true,
      request: {
        queries: {
          daily: {
            metric: 'future-metric',
            period: 'future-period',
            by: 'future-dimension',
            axis: 'demo-axis',
            agg: 'future-aggregation',
          },
        },
        formulas: { doubled: 'daily * 2' },
        viz: {
          type: 'future-viz',
          value: 'doubled',
          order: 'sideways',
          limit: -1,
        },
        time: { as_of: 'not-a-date' },
      },
      viz: {
        type: 'future-viz',
        value: 'doubled',
        order: 'sideways',
        limit: -1,
      },
      asOf: 'not-a-date',
    })
  })

  it.each([
    {
      name: 'a query with a non-string metric',
      source: VALID_WIDGET_DEFINITION.replace('metric: change', 'metric: 1'),
      message: 'queries.daily の形式が正しくありません。',
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
      name: 'a formula with a non-string value',
      source: VALID_WIDGET_DEFINITION.replace(
        'doubled: daily * 2',
        'doubled: 2',
      ),
      message: 'formulas は文字列のマップにしてください。',
    },
    {
      name: 'a widget with a non-string time.as_of',
      source: VALID_WIDGET_DEFINITION.replace(
        '  as_of: 2099-01-05',
        '  as_of: 1',
      ),
      message: 'time.as_of は文字列で指定してください。',
    },
    {
      name: 'a widget with time.range',
      source: VALID_WIDGET_DEFINITION.replace(
        '  as_of: 2099-01-05',
        '  as_of: 2099-01-05\n  range: 1d',
      ),
      message: 'time.range は現在のウィジェットでは使えません。',
    },
    {
      name: 'a widget with a null time.range',
      source: VALID_WIDGET_DEFINITION.replace(
        '  as_of: 2099-01-05',
        '  as_of: 2099-01-05\n  range: null',
      ),
      message: 'time.range は現在のウィジェットでは使えません。',
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
