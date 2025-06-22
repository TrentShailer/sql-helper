/// Trait for mapping certain postgres errors.
pub trait SqlError: Sized {
    /// Map a foreign key violation to a different error type.
    fn fk_violation<E, F: FnOnce() -> E>(self, f: F) -> Result<Self, E>;
    /// Map a unique violation to a different error type.
    fn unique_violation<E, F: FnOnce() -> E>(self, f: F) -> Result<Self, E>;
}

impl<T> SqlError for Result<T, postgres::Error> {
    fn fk_violation<E, F: FnOnce() -> E>(self, f: F) -> Result<Self, E> {
        if let Err(error) = &self {
            if let Some(sql_error) = error.code() {
                if sql_error == &postgres::error::SqlState::FOREIGN_KEY_VIOLATION {
                    return Err(f());
                }
            }
        }

        Ok(self)
    }

    fn unique_violation<E, F: FnOnce() -> E>(self, f: F) -> Result<Self, E> {
        if let Err(error) = &self {
            if let Some(sql_error) = error.code() {
                if sql_error == &postgres::error::SqlState::UNIQUE_VIOLATION {
                    return Err(f());
                }
            }
        }

        Ok(self)
    }
}
