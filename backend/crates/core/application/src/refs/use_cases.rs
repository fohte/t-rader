use std::collections::HashMap;

use core_domain::note_reference::is_valid_ref_id_format;

use crate::unit_of_work::SharedUnitOfWork;

use super::error::RefUseCaseError;
use super::repository::SharedRefRepository;
use super::types::{IndicatorRef, RefKind, RefSearchMatch, ResolvedRef, StockRef};

const AGENT_TERM_ORIGIN: &str = "llm";

#[derive(Clone)]
pub struct RefUseCases {
    unit_of_work: SharedUnitOfWork,
    repository: SharedRefRepository,
}

impl RefUseCases {
    pub fn new(unit_of_work: SharedUnitOfWork, repository: SharedRefRepository) -> Self {
        Self {
            unit_of_work,
            repository,
        }
    }

    pub async fn list_stocks(&self, query: Option<&str>) -> Result<Vec<StockRef>, RefUseCaseError> {
        let query = query
            .filter(|value| !value.is_empty())
            .map(super::sanitize_like);
        self.repository
            .list_stocks(query.as_deref())
            .await
            .map_err(Into::into)
    }

    pub async fn get_stock(&self, id: &str) -> Result<Option<StockRef>, RefUseCaseError> {
        self.repository.find_stock(id).await.map_err(Into::into)
    }

    pub async fn list_indicators(
        &self,
        query: Option<&str>,
    ) -> Result<Vec<IndicatorRef>, RefUseCaseError> {
        let query = query
            .filter(|value| !value.is_empty())
            .map(super::sanitize_like);
        self.repository
            .list_indicators(query.as_deref())
            .await
            .map_err(Into::into)
    }

    pub async fn get_indicator(&self, id: &str) -> Result<Option<IndicatorRef>, RefUseCaseError> {
        self.repository.find_indicator(id).await.map_err(Into::into)
    }

    pub async fn search_all(
        &self,
        query: &str,
        limit: u64,
    ) -> Result<Vec<RefSearchMatch>, RefUseCaseError> {
        let query = query.trim();
        if query.is_empty() {
            return Err(RefUseCaseError::Validation(
                "query must not be empty".into(),
            ));
        }
        let pattern = format!("%{}%", super::sanitize_like(&super::normalize_term(query)));
        self.repository
            .search_all(&pattern, limit)
            .await
            .map_err(Into::into)
    }

    pub async fn resolve(
        &self,
        requested: &[(String, String)],
    ) -> Result<Vec<ResolvedRef>, RefUseCaseError> {
        for (kind, id) in requested {
            if RefKind::try_from(kind.as_str()).is_err() {
                return Err(RefUseCaseError::Validation(format!(
                    "unknown ref kind: {kind}"
                )));
            }
            if !is_valid_ref_id_format(kind, id.trim()) {
                return Err(RefUseCaseError::Validation(format!(
                    "invalid group ref_id: {id}"
                )));
            }
        }

        let mut names = self.fetch_names(&group_by_kind(requested)).await?;
        let mut unresolved_by_kind = HashMap::new();
        for (kind, id) in requested {
            if !names.contains_key(&(kind.clone(), id.clone())) {
                unresolved_by_kind
                    .entry(kind.clone())
                    .or_insert_with(Vec::new)
                    .push(id.clone());
            }
        }

        let mut alias_targets = HashMap::new();
        for (kind, ids) in &unresolved_by_kind {
            let terms = self.repository.list_terms(kind).await?;
            let mut candidates_by_term: HashMap<String, Vec<String>> = HashMap::new();
            for row in terms {
                let candidates = candidates_by_term
                    .entry(super::normalize_term(&row.term))
                    .or_default();
                if !candidates.contains(&row.ref_id) {
                    candidates.push(row.ref_id);
                }
            }
            for id in ids {
                if let Some([target_id]) = candidates_by_term
                    .get(&super::normalize_term(id))
                    .map(Vec::as_slice)
                {
                    alias_targets.insert((kind.clone(), id.clone()), target_id.clone());
                }
            }
        }

        let alias_target_ids = group_by_kind(
            &alias_targets
                .iter()
                .map(|((kind, _), target_id)| (kind.clone(), target_id.clone()))
                .collect::<Vec<_>>(),
        );
        if !alias_target_ids.is_empty() {
            let target_names = self.fetch_names(&alias_target_ids).await?;
            alias_targets.retain(|(kind, _), target_id| {
                target_names.contains_key(&(kind.clone(), target_id.clone()))
            });
            names.extend(target_names);
        }

        Ok(requested
            .iter()
            .map(|(kind, id)| {
                let key = (kind.clone(), id.clone());
                if let Some(name) = names.get(&key) {
                    return ResolvedRef {
                        kind: kind.clone(),
                        id: id.clone(),
                        name: Some(name.clone()),
                    };
                }
                if let Some(target_id) = alias_targets.get(&key) {
                    return ResolvedRef {
                        kind: kind.clone(),
                        id: target_id.clone(),
                        name: names.get(&(kind.clone(), target_id.clone())).cloned(),
                    };
                }
                ResolvedRef {
                    kind: kind.clone(),
                    id: id.clone(),
                    name: None,
                }
            })
            .collect())
    }

    pub async fn add_terms(
        &self,
        ref_kind: &str,
        ref_id: &str,
        terms: &[String],
    ) -> Result<Vec<String>, RefUseCaseError> {
        let (ref_kind, ref_id) = normalize_ref(ref_kind, ref_id)?;
        let terms = normalize_terms(terms);
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let transaction = self.unit_of_work.begin().await?;
        let mut added = Vec::new();
        for term in terms {
            if self
                .repository
                .insert_term(&transaction, &ref_kind, &ref_id, &term, AGENT_TERM_ORIGIN)
                .await?
            {
                added.push(term);
            }
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(added)
    }

    pub async fn remove_terms(
        &self,
        ref_kind: &str,
        ref_id: &str,
        terms: &[String],
    ) -> Result<Vec<String>, RefUseCaseError> {
        let (ref_kind, ref_id) = normalize_ref(ref_kind, ref_id)?;
        let terms = normalize_terms(terms);
        if terms.is_empty() {
            return Ok(Vec::new());
        }

        let transaction = self.unit_of_work.begin().await?;
        let mut removed = Vec::new();
        for term in terms {
            if self
                .repository
                .delete_term(&transaction, &ref_kind, &ref_id, &term)
                .await?
            {
                removed.push(term);
            }
        }
        self.unit_of_work.commit(transaction).await?;
        Ok(removed)
    }

    async fn fetch_names(
        &self,
        ids_by_kind: &HashMap<String, Vec<String>>,
    ) -> Result<HashMap<(String, String), String>, RefUseCaseError> {
        let mut names = HashMap::new();
        for (kind, ids) in ids_by_kind {
            let by_id = match RefKind::try_from(kind.as_str()) {
                Ok(RefKind::Stock) => self.repository.stock_names(ids).await?,
                Ok(RefKind::Indicator) => self.repository.indicator_names(ids).await?,
                Ok(RefKind::Group) => self.repository.group_names(ids).await?,
                Err(_) => {
                    return Err(RefUseCaseError::Validation(format!(
                        "unknown ref kind: {kind}"
                    )));
                }
            };
            names.extend(
                by_id
                    .into_iter()
                    .map(|(id, name)| ((kind.clone(), id), name)),
            );
        }
        Ok(names)
    }
}

fn normalize_ref(ref_kind: &str, ref_id: &str) -> Result<(String, String), RefUseCaseError> {
    let ref_kind = ref_kind.trim();
    if RefKind::try_from(ref_kind).is_err() {
        return Err(RefUseCaseError::Validation(format!(
            "invalid ref_kind: {ref_kind}"
        )));
    }
    let ref_id = ref_id.trim();
    if ref_id.is_empty() {
        return Err(RefUseCaseError::Validation(
            "ref_id must not be empty".into(),
        ));
    }
    if !is_valid_ref_id_format(ref_kind, ref_id) {
        return Err(RefUseCaseError::Validation(format!(
            "invalid group ref_id: {ref_id}"
        )));
    }
    Ok((ref_kind.to_string(), ref_id.to_string()))
}

fn normalize_terms(terms: &[String]) -> Vec<String> {
    terms
        .iter()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .map(str::to_string)
        .collect()
}

fn group_by_kind(requested: &[(String, String)]) -> HashMap<String, Vec<String>> {
    let mut grouped = HashMap::new();
    for (kind, id) in requested {
        grouped
            .entry(kind.clone())
            .or_insert_with(Vec::new)
            .push(id.clone());
    }
    grouped
}
