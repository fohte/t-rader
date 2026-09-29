use core_application::persistence::PersistenceError;
use sea_orm::{DbErr, RuntimeErr, SqlErr};

pub(crate) fn persistence_error(error: DbErr) -> PersistenceError {
    let message = error.to_string();
    if matches!(&error, DbErr::RecordNotUpdated) {
        return PersistenceError::RecordNotUpdated(message);
    }
    if let Some(sql_error) = error.sql_err() {
        match sql_error {
            SqlErr::ForeignKeyConstraintViolation(_) => {
                return PersistenceError::MissingReference(message);
            }
            SqlErr::UniqueConstraintViolation(_) => {
                return PersistenceError::Conflict(message);
            }
            _ => {}
        }
    }
    let code = match &error {
        DbErr::Exec(RuntimeErr::SqlxError(error)) | DbErr::Query(RuntimeErr::SqlxError(error)) => {
            error.as_database_error().and_then(|error| error.code())
        }
        _ => None,
    };
    match code.as_deref() {
        Some("23503") => PersistenceError::MissingReference(message),
        Some("23505") => PersistenceError::Conflict(message),
        Some("23514") => PersistenceError::ConstraintViolation(message),
        _ => PersistenceError::Database(message),
    }
}
