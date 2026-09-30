#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteKind {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNoteKind {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: i32,
}

#[derive(Debug, Clone)]
pub struct CreateNoteKindCommand {
    pub key: String,
    pub display_name: String,
    pub requires_approval: bool,
    pub description: Option<String>,
    pub sort_order: Option<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateNoteKindCommand {
    pub display_name: Option<String>,
    pub requires_approval: Option<bool>,
    pub description: Option<Option<String>>,
    pub sort_order: Option<i32>,
}
