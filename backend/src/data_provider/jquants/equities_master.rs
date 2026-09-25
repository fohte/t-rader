use async_trait::async_trait;
use core_domain::equity_master::EquityMasterEntry;

use super::JQuantsClient;
use super::response::EquitiesMasterResponse;
use crate::data_provider::{DataProviderError, EquityMasterSource, EquityMasterSourceError};

#[async_trait]
impl EquityMasterSource for JQuantsClient {
    async fn fetch_all_equities_master(
        &self,
    ) -> Result<Vec<EquityMasterEntry>, EquityMasterSourceError> {
        Ok(JQuantsClient::fetch_all_equities_master(self).await?)
    }
}

impl JQuantsClient {
    /// `/equities/master` をパラメータ無しで呼び出し、実行日時点の全上場銘柄を取得する。
    pub(crate) async fn fetch_all_equities_master(
        &self,
    ) -> Result<Vec<EquityMasterEntry>, DataProviderError> {
        let url = self.build_url("/equities/master", &[])?;

        tracing::debug!(%url, "J-Quants API から全銘柄マスタを取得中");

        let response = self.get_with_retry(&url, self.current_rate_limit()).await?;
        let body: EquitiesMasterResponse = response
            .json()
            .await
            .map_err(|e| DataProviderError::Parse(e.to_string()))?;

        Ok(body
            .data
            .into_iter()
            .map(|m| EquityMasterEntry {
                id: super::normalize_local_code(&m.code).to_string(),
                name: m.company_name,
                market: non_empty(m.market_name),
                sector_name: non_empty(m.sector_name),
                product_category: non_empty(m.product_category),
            })
            .collect())
    }
}

/// 空文字列を「値なし」として `None` に正規化する。J-Quants は該当なしを空文字列で
/// 返す場合があるため、`Some("")` のまま DB に書き込むと空文字の sector 行が
/// 作られてしまう。
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|s| !s.trim().is_empty())
}
