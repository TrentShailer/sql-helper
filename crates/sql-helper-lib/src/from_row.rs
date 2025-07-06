use postgres::Row;

/// Convert a row to an instance of self.
pub trait FromRow: Sized {
    /// Try convert a row to an instance of self.
    #[track_caller]
    fn from_row(row: &Row) -> Result<Self, postgres::Error>;
}
