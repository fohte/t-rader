use thiserror::Error;

use crate::instrument::Market;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignStockId {
    value: String,
    country: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ForeignStockIdError {
    #[error("country must be an assigned ISO 3166-1 alpha-2 code")]
    InvalidCountry,
    #[error("Japanese stocks must not use a country prefix")]
    JapaneseCountry,
    #[error("code must contain only uppercase ASCII letters, digits, or hyphens")]
    InvalidCode,
}

impl ForeignStockId {
    pub fn new(country: &str, code: &str) -> Result<Self, ForeignStockIdError> {
        if country == "JP" {
            return Err(ForeignStockIdError::JapaneseCountry);
        }
        if !SUPPORTED_FOREIGN_COUNTRY_CODES
            .split_whitespace()
            .any(|candidate| candidate == country)
        {
            return Err(ForeignStockIdError::InvalidCountry);
        }
        if code.is_empty()
            || !code
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'-')
        {
            return Err(ForeignStockIdError::InvalidCode);
        }

        Ok(Self {
            value: format!("{country}:{code}"),
            country: country.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn market(&self) -> Market {
        if self.country == "US" {
            Market::Us
        } else {
            Market::Other
        }
    }
}

const SUPPORTED_FOREIGN_COUNTRY_CODES: &str = "AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI VN VU WF WS YE YT ZA ZM ZW";

pub fn has_country_prefix(stock_id: &str) -> bool {
    stock_id.contains(':')
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::{ForeignStockId, ForeignStockIdError, has_country_prefix};
    use crate::instrument::Market;

    #[rstest]
    #[case::us("US", "QZ-7", "US:QZ-7", Market::Us)]
    #[case::other_country("KR", "QZ9012", "KR:QZ9012", Market::Other)]
    fn constructs_foreign_stock_ids(
        #[case] country: &str,
        #[case] code: &str,
        #[case] expected_id: &str,
        #[case] expected_market: Market,
    ) {
        let stock_id = ForeignStockId::new(country, code).expect("valid foreign stock ID");

        assert_eq!(
            (stock_id.as_str().to_owned(), stock_id.market()),
            (expected_id.to_owned(), expected_market),
        );
    }

    #[rstest]
    #[case::japan("JP", "QZ7", ForeignStockIdError::JapaneseCountry)]
    #[case::lowercase_country("us", "QZ7", ForeignStockIdError::InvalidCountry)]
    #[case::short_country("U", "QZ7", ForeignStockIdError::InvalidCountry)]
    #[case::unassigned_country("ZZ", "QZ7", ForeignStockIdError::InvalidCountry)]
    #[case::non_ascii_country("ＵＳ", "QZ7", ForeignStockIdError::InvalidCountry)]
    #[case::empty_code("US", "", ForeignStockIdError::InvalidCode)]
    #[case::lowercase_code("US", "qz7", ForeignStockIdError::InvalidCode)]
    #[case::invalid_code_character("US", "QZ.7", ForeignStockIdError::InvalidCode)]
    fn rejects_invalid_foreign_stock_ids(
        #[case] country: &str,
        #[case] code: &str,
        #[case] expected_error: ForeignStockIdError,
    ) {
        assert_eq!(ForeignStockId::new(country, code), Err(expected_error));
    }

    #[rstest]
    #[case::foreign("US:QZ-7", true)]
    #[case::japanese("1234", false)]
    fn detects_country_prefixed_stock_ids(#[case] stock_id: &str, #[case] expected: bool) {
        assert_eq!(has_country_prefix(stock_id), expected);
    }
}
