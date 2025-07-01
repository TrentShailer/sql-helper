#![allow(missing_docs)]

use core::{error::Error, fmt};

use ts_sql_helper_derive::{FromRow, FromSql};

#[derive(FromRow)]
pub struct TestStruct {
    pub field_a: String,
    pub field_b: TestEnum,
}

#[derive(FromSql)]
#[repr(i8)]
pub enum TestEnum {
    A = 0,
    B = 1,
    C = 2,
}
impl TryFrom<i8> for TestEnum {
    type Error = TryFromStringError;

    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::A),
            1 => Ok(Self::B),
            2 => Ok(Self::C),
            _ => Err(TryFromStringError),
        }
    }
}

#[derive(Debug)]
pub struct TryFromStringError;
impl fmt::Display for TryFromStringError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "value is not a valid instance of `TestEnum`")
    }
}
impl Error for TryFromStringError {}
