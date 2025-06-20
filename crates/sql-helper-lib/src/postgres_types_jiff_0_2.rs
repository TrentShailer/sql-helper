//! Adapted from https://github.com/sfackler/rust-postgres/blob/e1cd6beef3a1530642a2abaf3584d6bd8ed6cd45/postgres-types/src/jiff_02.rs
//! At https://github.com/sfackler/rust-postgres/commit/cd8a34199c65f3e28c9466f1ab59949ce5d15509
//! Licensed under MIT or Apache-2.0

use bytes::BytesMut;
use jiff::{
    Span, SpanRound, Timestamp, Unit,
    civil::{Date, DateTime, Time},
};
use postgres::types::{FromSql, IsNull, ToSql, Type, accepts, to_sql_checked};
use postgres_protocol::types;
use serde::{Deserialize, Serialize};
use std::error::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Wrapper for [`jiff::Timestamp`]
pub struct SqlTimestamp(pub Timestamp);

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Wrapper for [`jiff::civil::Date`]
pub struct SqlDate(pub Date);

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Wrapper for [`jiff::civil::DateTime`]
pub struct SqlDateTime(pub DateTime);

#[derive(Debug, Clone, Serialize, Deserialize)]
/// Wrapper for [`jiff::civil::Time`]
pub struct SqlTime(pub Time);

const fn base() -> DateTime {
    DateTime::constant(2000, 1, 1, 0, 0, 0, 0)
}

/// The number of seconds from the Unix epoch to 2000-01-01 00:00:00 UTC.
const PG_EPOCH: i64 = 946684800;

fn base_ts() -> Timestamp {
    Timestamp::new(PG_EPOCH, 0).unwrap()
}

fn round_us<'a>() -> SpanRound<'a> {
    SpanRound::new().largest(Unit::Microsecond)
}

fn decode_err<E>(_e: E) -> Box<dyn Error + Sync + Send>
where
    E: Error,
{
    "value too large to decode".into()
}

fn transmit_err<E>(_e: E) -> Box<dyn Error + Sync + Send>
where
    E: Error,
{
    "value too large to transmit".into()
}

impl<'a> FromSql<'a> for SqlDateTime {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let v = types::timestamp_from_sql(raw)?;
        Ok(Self(
            Span::new()
                .try_microseconds(v)
                .and_then(|s| base().checked_add(s))
                .map_err(decode_err)?,
        ))
    }

    accepts!(TIMESTAMP);
}

impl ToSql for SqlDateTime {
    fn to_sql(&self, _: &Type, w: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let v = self
            .0
            .since(base())
            .and_then(|s| s.round(round_us().relative(base())))
            .map_err(transmit_err)?
            .get_microseconds();
        types::timestamp_to_sql(v, w);
        Ok(IsNull::No)
    }

    accepts!(TIMESTAMP);
    to_sql_checked!();
}

impl<'a> FromSql<'a> for SqlTimestamp {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let v = types::timestamp_from_sql(raw)?;
        Ok(Self(
            Span::new()
                .try_microseconds(v)
                .and_then(|s| base_ts().checked_add(s))
                .map_err(decode_err)?,
        ))
    }

    accepts!(TIMESTAMPTZ);
}

impl ToSql for SqlTimestamp {
    fn to_sql(&self, _: &Type, w: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let v = self
            .0
            .since(base_ts())
            .and_then(|s| s.round(round_us()))
            .map_err(transmit_err)?
            .get_microseconds();
        types::timestamp_to_sql(v, w);
        Ok(IsNull::No)
    }

    accepts!(TIMESTAMPTZ);
    to_sql_checked!();
}

impl<'a> FromSql<'a> for SqlDate {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let v = types::date_from_sql(raw)?;
        Ok(Self(
            Span::new()
                .try_days(v)
                .and_then(|s| base().date().checked_add(s))
                .map_err(decode_err)?,
        ))
    }
    accepts!(DATE);
}

impl ToSql for SqlDate {
    fn to_sql(&self, _: &Type, w: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let v = self
            .0
            .since(base().date())
            .map_err(transmit_err)?
            .get_days();
        types::date_to_sql(v, w);
        Ok(IsNull::No)
    }

    accepts!(DATE);
    to_sql_checked!();
}

impl<'a> FromSql<'a> for SqlTime {
    fn from_sql(_: &Type, raw: &[u8]) -> Result<Self, Box<dyn Error + Sync + Send>> {
        let v = types::time_from_sql(raw)?;
        Ok(Self(
            Span::new()
                .try_microseconds(v)
                .and_then(|s| Time::midnight().checked_add(s))
                .map_err(decode_err)?,
        ))
    }

    accepts!(TIME);
}

impl ToSql for SqlTime {
    fn to_sql(&self, _: &Type, w: &mut BytesMut) -> Result<IsNull, Box<dyn Error + Sync + Send>> {
        let v = self
            .0
            .since(Time::midnight())
            .and_then(|s| s.round(round_us()))
            .map_err(transmit_err)?
            .get_microseconds();
        types::time_to_sql(v, w);
        Ok(IsNull::No)
    }

    accepts!(TIME);
    to_sql_checked!();
}
