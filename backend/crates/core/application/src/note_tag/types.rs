#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteTag {
    pub key: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNoteTag {
    pub key: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CreateNoteTagCommand {
    pub key: String,
    pub display_name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct UpdateNoteTagCommand {
    pub display_name: Option<String>,
    pub description: Option<Option<String>>,
}
