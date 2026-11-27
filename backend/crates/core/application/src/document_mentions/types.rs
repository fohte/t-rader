use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentMention {
    pub chunk_id: Uuid,
    pub stock_id: String,
    pub start_offset: i32,
    pub end_offset: i32,
}
