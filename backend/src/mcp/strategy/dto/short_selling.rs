//! `read_short_sale_reports` / `read_sector_short_ratio` の入出力スキーマ。

use chrono::NaiveDate;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadShortSaleReportsParams {
    /// 対象銘柄コード (4桁、例: "7203")
    pub symbol: String,
    /// 公表日の下限 (YYYY-MM-DD, inclusive)。省略時は下限なし
    pub from: Option<NaiveDate>,
    /// 公表日の上限 (YYYY-MM-DD, inclusive)。省略時は上限なし
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}

/// 空売り残高報告 (J-Quants `/markets/short-sale-report`) の 1 件。報告義務は残高割合
/// 0.5% 以上の空売りにのみ生じるため、この行が無いことは「空売りが無い」ことではなく
/// 「報告義務のある空売りが無い」ことしか意味しない。
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ShortSaleReportDto {
    /// 公表日
    pub disc_date: NaiveDate,
    /// 残高を計算した基準日
    pub calc_date: NaiveDate,
    /// 空売りをした者の商号・名称・氏名。和文/英文の表記が混在する
    pub reporter_name: String,
    /// 空売りをした者の住所。記載が無ければ null
    pub reporter_address: Option<String>,
    /// 委託者、または投資一任契約の相手方の名称。該当が無ければ null
    pub client_name: Option<String>,
    /// 上記の住所。該当が無ければ null
    pub client_address: Option<String>,
    /// 信託財産の名称。該当が無ければ null
    pub fund_name: Option<String>,
    /// 空売り残高割合 (小数。0.01 = 1%)
    pub short_position_ratio: f64,
    pub short_position_shares: i64,
    pub short_position_units: i64,
    /// 直近報告の計算基準日。初回報告は null
    pub prev_report_date: Option<NaiveDate>,
    /// 直近報告の残高割合 (小数)。初回報告は null。`short_position_ratio` との差分が
    /// 前回報告からの増減
    pub prev_report_ratio: Option<f64>,
    /// 備考。無ければ null
    pub notes: Option<String>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadShortSaleReportsResult {
    pub symbol: String,
    /// 公表日の新しい順 (同一公表日内は報告者名の昇順)
    pub items: Vec<ShortSaleReportDto>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReadSectorShortRatioParams {
    /// 業種名 (`sector` テーブル / `search_refs` / `check_buyable_qty` と同じ表記の 33 業種名。例: "輸送用機器")
    pub sector: String,
    /// 対象日の下限 (YYYY-MM-DD, inclusive)。省略時は下限なし
    pub from: Option<NaiveDate>,
    /// 対象日の上限 (YYYY-MM-DD, inclusive)。省略時は上限なし
    pub to: Option<NaiveDate>,
    pub limit: Option<u32>,
}

/// 業種別空売り比率 (J-Quants `/markets/short-ratio`) の 1 日分。その業種で売買が無かった
/// 日は 4 フィールドすべて null
#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct SectorShortRatioDto {
    pub date: NaiveDate,
    /// 実注文 (空売りでない通常の売り) の売買代金 (円)
    pub sell_excluding_short_value: Option<f64>,
    /// 価格規制ありの空売りの売買代金 (円)
    pub short_with_restriction_value: Option<f64>,
    /// 価格規制なしの空売りの売買代金 (円)
    pub short_without_restriction_value: Option<f64>,
    /// 空売り比率: 空売り (価格規制あり+なし) の売買代金 / (実注文+空売り) の売買代金合計
    /// (小数。0.1 = 10%)
    pub short_ratio: Option<f64>,
}

#[derive(Debug, Serialize, JsonSchema, PartialEq)]
pub struct ReadSectorShortRatioResult {
    pub sector: String,
    /// 日付の新しい順
    pub items: Vec<SectorShortRatioDto>,
}
