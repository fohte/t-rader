use serde::{Deserialize, Deserializer};

/// `Option<Option<T>>` でフィールド省略と明示的な `null` を区別する。
pub(crate) fn deserialize_nullable_option<'de, D, T>(
    deserializer: D,
) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: Deserializer<'de>,
{
    Option::<T>::deserialize(deserializer).map(Some)
}
