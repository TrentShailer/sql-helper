use postgres::Row;

/// Convert a row to an instance of self.
pub trait FromRow: Sized {
    /// Try convert a row to an instance of self.
    #[track_caller]
    fn from_row(row: &Row) -> Result<Self, postgres::Error>;
}

/// Parse a type from a row.
pub trait ParseFromRow: Sized {
    /// Parse the row into type `T`.
    #[track_caller]
    fn parse<T>(&self) -> Result<T, postgres::Error>
    where
        T: FromRow;
}

impl ParseFromRow for Row {
    #[track_caller]
    fn parse<T>(&self) -> Result<T, postgres::Error>
    where
        T: FromRow,
    {
        T::from_row(self)
    }
}
