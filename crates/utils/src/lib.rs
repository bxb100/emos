use std::fmt::Write;

use anyhow::Result;
use anyhow::bail;

pub trait SqlInClause {
    fn to_sql_in_clause(&self) -> Result<String>;
}

impl SqlInClause for Vec<i64> {
    fn to_sql_in_clause(&self) -> Result<String> {
        if self.is_empty() {
            bail!("Cannot create SQL IN clause from empty vector");
        }
        let mut id_str = String::with_capacity(self.len() * 10);
        for (i, id) in self.iter().enumerate() {
            if i > 0 {
                id_str.push(',');
            }
            write!(id_str, "{}", id)?;
        }
        Ok(id_str)
    }
}
