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
        // PostgreSQL 18 は ON DELETE RESTRICT に専用の SQLSTATE を返す。
        Some("23001" | "23503") => PersistenceError::MissingReference(message),
        Some("23505") => PersistenceError::Conflict(message),
        Some("23514") => PersistenceError::ConstraintViolation(message),
        _ => PersistenceError::Database(message),
    }
}

#[cfg(test)]
mod tests {
    use std::{
        borrow::Cow,
        error::Error as StdError,
        fmt::{self, Display, Formatter},
        sync::Arc,
    };

    use core_application::persistence::PersistenceError;
    use rstest::rstest;
    use sea_orm::{DbErr, RuntimeErr};
    use sqlx::error::{DatabaseError, ErrorKind};

    use super::persistence_error;

    #[derive(Debug)]
    struct SqlStateError(&'static str);

    impl Display for SqlStateError {
        fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
            formatter.write_str("synthetic database error")
        }
    }

    impl StdError for SqlStateError {}

    impl DatabaseError for SqlStateError {
        fn message(&self) -> &str {
            "synthetic database error"
        }

        fn code(&self) -> Option<Cow<'_, str>> {
            Some(Cow::Borrowed(self.0))
        }

        fn as_error(&self) -> &(dyn StdError + Send + Sync + 'static) {
            self
        }

        fn as_error_mut(&mut self) -> &mut (dyn StdError + Send + Sync + 'static) {
            self
        }

        fn into_error(self: Box<Self>) -> Box<dyn StdError + Send + Sync + 'static> {
            self
        }

        fn kind(&self) -> ErrorKind {
            ErrorKind::Other
        }
    }

    fn database_error_with_sqlstate(sqlstate: &'static str) -> DbErr {
        DbErr::Exec(RuntimeErr::SqlxError(Arc::new(sqlx::Error::Database(
            Box::new(SqlStateError(sqlstate)),
        ))))
    }

    #[rstest]
    #[case::restrict_violation("23001")]
    #[case::foreign_key_violation("23503")]
    fn classifies_reference_constraint_sqlstates_as_missing_reference(
        #[case] sqlstate: &'static str,
    ) {
        assert_eq!(
            match persistence_error(database_error_with_sqlstate(sqlstate)) {
                PersistenceError::MissingReference(message) => Some(message),
                _ => None,
            },
            Some("Execution Error: error returned from database: synthetic database error".into()),
        );
    }
}
