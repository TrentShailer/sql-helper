use postgres_types::Type;
use syn::parse_quote;

pub fn get_param_types(sql: &str) -> Vec<Type> {
    let mut parameter_types = vec![];

    enum State {
        Neutral,
        ConsumingNumber { has_consumed_a_digit: bool },
        ConsumingTypeSeparator,
        ConsumingType { type_string: String },
    }

    let mut state = State::Neutral;
    for character in sql.chars() {
        match &mut state {
            State::Neutral => {
                if character == '$' {
                    state = State::ConsumingNumber {
                        has_consumed_a_digit: false,
                    };
                }
            }
            State::ConsumingNumber {
                has_consumed_a_digit,
            } => {
                if character.is_ascii_digit() {
                    *has_consumed_a_digit = true;
                } else if character == ':' {
                    state = State::ConsumingTypeSeparator;
                } else {
                    if *has_consumed_a_digit {
                        parameter_types.push("unknown".to_string());
                    }
                    state = State::Neutral;
                }
            }
            State::ConsumingTypeSeparator => {
                if character.is_ascii_alphabetic() {
                    state = State::ConsumingType {
                        type_string: character.to_string(),
                    };
                } else if character != ':' {
                    parameter_types.push("unknown".to_string());
                    state = State::Neutral;
                }
            }
            State::ConsumingType { type_string } => {
                if character.is_ascii_alphanumeric() || character == '[' || character == ']' {
                    type_string.push(character);
                } else {
                    parameter_types.push(type_string.to_uppercase());
                    state = State::Neutral;
                }
            }
        }
    }
    match state {
        State::Neutral => {}
        State::ConsumingNumber {
            has_consumed_a_digit,
        } => {
            if has_consumed_a_digit {
                parameter_types.push("unknown".to_string());
            }
        }
        State::ConsumingTypeSeparator => {
            parameter_types.push("unknown".to_string());
        }
        State::ConsumingType { type_string } => {
            parameter_types.push(type_string.to_uppercase());
        }
    }

    parameter_types
        .into_iter()
        .map(|type_string| match type_string.as_str() {
            "BOOL" => Type::BOOL,
            "BOOL[]" => Type::BOOL_ARRAY,
            "BYTEA" => Type::BYTEA,
            "BYTEA[]" => Type::BYTEA_ARRAY,
            "CHAR" => Type::CHAR,
            "CHAR[]" => Type::CHAR_ARRAY,
            "INT8" => Type::INT8,
            "INT8[]" => Type::INT8_ARRAY,
            "INT4" => Type::INT4,
            "INT4[]" => Type::INT4_ARRAY,
            "INT2" => Type::INT2,
            "INT2[]" => Type::INT2_ARRAY,
            "FLOAT8" => Type::FLOAT8,
            "FLOAT8[]" => Type::FLOAT8_ARRAY,
            "FLOAT4" => Type::FLOAT4,
            "FLOAT4[]" => Type::FLOAT4_ARRAY,
            "UUID" => Type::UUID,
            "UUID[]" => Type::UUID_ARRAY,
            "TEXT" => Type::TEXT,
            "VARCHAR" => Type::VARCHAR,
            "VARCHAR[]" => Type::VARCHAR_ARRAY,
            "TEXT[]" => Type::TEXT_ARRAY,
            "TIMESTAMP" => Type::TIMESTAMP,
            "TIMESTAMP[]" => Type::TIMESTAMP_ARRAY,
            "TIMESTAMPTZ" => Type::TIMESTAMPTZ,
            "TIMESTAMPTZ[]" => Type::TIMESTAMPTZ_ARRAY,
            "DATE" => Type::DATE,
            "DATE[]" => Type::DATE_ARRAY,
            "TIME" => Type::TIME,
            "TIME[]" => Type::TIME_ARRAY,
            type_string => {
                panic!("unsupported type `{type_string}`")
            }
        })
        .collect()
}

pub fn parameter_to_type(parameter_type: &Type) -> syn::Type {
    match parameter_type {
        &Type::BOOL => parse_quote!(&'a bool),
        &Type::BOOL_ARRAY => parse_quote!(&'a [bool]),
        &Type::BYTEA => parse_quote!(&'a [u8]),
        &Type::BYTEA_ARRAY => parse_quote!(&'a [Vec<u8>]),
        &Type::CHAR => parse_quote!(&'a i8),
        &Type::CHAR_ARRAY => parse_quote!(&'a [i8]),
        &Type::INT8 => parse_quote!(&'a i64),
        &Type::INT8_ARRAY => parse_quote!(&'a [i64]),
        &Type::INT4 => parse_quote!(&'a i32),
        &Type::INT4_ARRAY => parse_quote!(&'a [i32]),
        &Type::INT2 => parse_quote!(&'a i16),
        &Type::INT2_ARRAY => parse_quote!(&'a [i16]),
        &Type::FLOAT8 => parse_quote!(&'a f64),
        &Type::FLOAT8_ARRAY => parse_quote!(&'a [f64]),
        &Type::FLOAT4 => parse_quote!(&'a f32),
        &Type::FLOAT4_ARRAY => parse_quote!(&'a [f32]),
        &Type::UUID => parse_quote!(&'a uuid::Uuid),
        &Type::UUID_ARRAY => parse_quote!(&'a [uuid::Uuid]),
        &Type::TEXT | &Type::VARCHAR => parse_quote!(&'a str),
        &Type::VARCHAR_ARRAY | &Type::TEXT_ARRAY => parse_quote!(&'a [String]),
        &Type::TIMESTAMP => parse_quote!(&'a ts_sql_helper_lib::SqlDateTime),
        &Type::TIMESTAMP_ARRAY => parse_quote!(&'a [ts_sql_helper_lib::SqlDateTime]),
        &Type::TIMESTAMPTZ => parse_quote!(&'a ts_sql_helper_lib::SqlTimestamp),
        &Type::TIMESTAMPTZ_ARRAY => parse_quote!(&'a [ts_sql_helper_lib::SqlTimestamp]),
        &Type::DATE => parse_quote!(&'a ts_sql_helper_lib::SqlDate),
        &Type::DATE_ARRAY => parse_quote!(&'a [ts_sql_helper_lib::SqlDate]),
        &Type::TIME => parse_quote!(&'a ts_sql_helper_lib::SqlTime),
        &Type::TIME_ARRAY => parse_quote!(&'a [ts_sql_helper_lib::SqlTime]),
        _ => unreachable!(),
    }
}
