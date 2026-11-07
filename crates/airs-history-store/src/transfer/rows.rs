//! Column metadata comes exclusively from the admitted compiled schema, never input SQL.
use super::codec::{Row, Table};
use crate::{database::sql_error, failure};
use airs_runtime_core::error::CliError;
use rusqlite::{
    Connection,
    types::{Value as SqlValue, ValueRef},
};
use serde_json::Value;

pub struct Column {
    pub name: String,
    pub integer: bool,
    pub required: bool,
    pub primary: i64,
}
pub struct Spec {
    pub table: Table,
    pub columns: Vec<Column>,
}
fn identifier(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
}
impl Spec {
    pub fn load(conn: &Connection, table: Table) -> Result<Self, CliError> {
        let mut statement = conn
            .prepare(&format!("PRAGMA table_info({})", table.name()))
            .map_err(sql_error)?;
        let columns = statement
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, bool>(3)?,
                    r.get::<_, i64>(5)?,
                ))
            })
            .map_err(sql_error)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sql_error)?;
        if columns.is_empty()
            || columns.len() > 32
            || columns.iter().any(|(name, kind, _, _)| {
                !identifier(name) || !["INTEGER", "TEXT"].contains(&kind.as_str())
            })
        {
            return Err(failure("unsupported trusted transfer schema"));
        }
        let columns = columns
            .into_iter()
            .map(|(name, kind, required, primary)| Column {
                name,
                integer: kind == "INTEGER",
                required: required || primary > 0,
                primary,
            })
            .collect();
        Ok(Self { table, columns })
    }
    pub fn select(&self) -> String {
        let names = self
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let mut keys = self
            .columns
            .iter()
            .filter(|c| c.primary > 0)
            .collect::<Vec<_>>();
        keys.sort_by_key(|c| c.primary);
        let order = keys
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        format!("SELECT {names} FROM {} ORDER BY {order}", self.table.name())
    }
    pub fn insert(&self) -> String {
        let names = self
            .columns
            .iter()
            .map(|c| c.name.as_str())
            .collect::<Vec<_>>()
            .join(",");
        let parameters = std::iter::repeat_n("?", self.columns.len())
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "INSERT INTO {} ({names}) VALUES ({parameters})",
            self.table.name()
        )
    }
    pub fn read(&self, row: &rusqlite::Row<'_>) -> Result<Row, CliError> {
        let mut values = Vec::with_capacity(self.columns.len());
        for n in 0..self.columns.len() {
            values.push(match row.get_ref(n).map_err(sql_error)? {
                ValueRef::Null => Value::Null,
                ValueRef::Integer(v) => Value::from(v),
                ValueRef::Text(v) => Value::String(
                    std::str::from_utf8(v)
                        .map_err(|_| failure("archive text is not valid UTF-8"))?
                        .into(),
                ),
                _ => {
                    return Err(failure(
                        "unsupported archive value type; no physical pages exported",
                    ));
                }
            });
        }
        Ok(Row {
            table: self.table,
            values,
        })
    }
    pub fn values(&self, row: &Row) -> Result<(Vec<SqlValue>, usize), CliError> {
        if row.table != self.table || row.values.len() != self.columns.len() {
            return Err(failure(
                "logical transfer column count does not match its record kind",
            ));
        }
        let mut bytes = 1024usize;
        let mut values = Vec::with_capacity(self.columns.len());
        for (column, value) in self.columns.iter().zip(&row.values) {
            values.push(if value.is_null() && !column.required {
                SqlValue::Null
            } else if column.integer {
                SqlValue::Integer(value.as_i64().ok_or_else(|| {
                    failure("logical transfer integer column has an invalid type")
                })?)
            } else {
                let text = value
                    .as_str()
                    .ok_or_else(|| failure("logical transfer text column has an invalid type"))?;
                if text.chars().any(char::is_control) {
                    return Err(failure(
                        "logical transfer text contains unsupported controls",
                    ));
                }
                bytes = bytes
                    .checked_add(text.len())
                    .ok_or_else(|| failure("logical transfer record exceeds its byte bound"))?;
                SqlValue::Text(text.into())
            });
        }
        if bytes > 256 * 1024 {
            return Err(failure(
                "logical transfer record exceeds the normalized 256 KiB batch bound",
            ));
        }
        Ok((values, bytes))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn logical_transfer_columns_reject_affinity_coercions_and_preserve_all_fixed_kinds() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::database::SCHEMA).unwrap();
        for table in Table::ALL {
            let spec = Spec::load(&conn, table).unwrap();
            assert!(!spec.select().contains(';'));
            assert!(!spec.insert().contains(';'));
            assert!(
                spec.values(&Row {
                    table,
                    values: vec![]
                })
                .is_err()
            );
            let mut valid = Row {
                table,
                values: spec
                    .columns
                    .iter()
                    .map(|c| {
                        if c.integer {
                            Value::from(1)
                        } else {
                            Value::String("synthetic".into())
                        }
                    })
                    .collect(),
            };
            spec.values(&valid).unwrap();
            let index = spec.columns.iter().position(|c| c.integer);
            if let Some(index) = index {
                valid.values[index] = Value::String("1".into());
                assert!(spec.values(&valid).is_err());
            }
        }
    }
}
