use std::collections::BTreeMap;

use core_application::widget::{WidgetDefinition, WidgetMetric};
use gateway_postgres::performance_workload_sql;
use sha2::{Digest, Sha256};

pub(super) fn fingerprint(sql: &str) -> String {
    let normalized = sql.split_whitespace().collect::<Vec<_>>().join(" ");
    let digest = Sha256::digest(normalized.trim_end_matches(';').trim_end().as_bytes());
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

pub(super) fn expected_sql_by_case(
    definitions: &[(&str, &WidgetDefinition)],
) -> Result<BTreeMap<String, Vec<String>>, String> {
    let signatures = performance_workload_sql::statements()
        .into_iter()
        .map(|(name, sql)| (name, fingerprint(sql)))
        .collect::<BTreeMap<_, _>>();
    let mut expected = BTreeMap::new();
    for (case_id, definition) in definitions {
        let query = definition
            .queries
            .values()
            .next()
            .ok_or_else(|| "widget case has no query".to_string())?;
        let resolve_signature = match query.metric {
            WidgetMetric::Indicator => "widget.resolve.indicator",
            WidgetMetric::Change
            | WidgetMetric::Close
            | WidgetMetric::Turnover
            | WidgetMetric::MktCap
            | WidgetMetric::Per
            | WidgetMetric::Pbr => match definition.time.as_of {
                None => "widget.resolve.latest",
                Some(_) => "widget.resolve.requested",
            },
        };
        let query_signature = match query.metric {
            WidgetMetric::Indicator => "widget.query.indicator",
            WidgetMetric::Change
            | WidgetMetric::Close
            | WidgetMetric::Turnover
            | WidgetMetric::MktCap
            | WidgetMetric::Per
            | WidgetMetric::Pbr => "widget.query",
        };
        let sql = [resolve_signature, query_signature]
            .into_iter()
            .map(|name| {
                signatures
                    .get(name)
                    .cloned()
                    .ok_or_else(|| format!("missing expected SQL signature: {name}"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        expected.insert((*case_id).to_string(), sql);
    }
    let group_sql = ["group-movers.axis-exists", "group-movers.list"]
        .into_iter()
        .map(|name| {
            signatures
                .get(name)
                .cloned()
                .ok_or_else(|| format!("missing expected SQL signature: {name}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    for case_id in [
        "group-movers-short",
        "group-movers-long",
        "group-movers-short-filtered",
        "group-movers-long-filtered",
    ] {
        expected.insert(case_id.to_string(), group_sql.clone());
    }
    Ok(expected)
}

pub(super) fn required_sql_fingerprints() -> Result<BTreeMap<&'static str, String>, String> {
    Ok(performance_workload_sql::statements()
        .into_iter()
        .map(|(name, sql)| (name, fingerprint(sql)))
        .collect::<BTreeMap<_, _>>())
}
