use std::fmt::Write;

use anyhow::Result;
use anyhow::bail;

pub trait SqlInClause {
    fn to_sql_in_clause(self) -> Result<String>;
}

impl<I> SqlInClause for I
where
    I: IntoIterator<Item = i64>,
{
    fn to_sql_in_clause(self) -> Result<String> {
        let iter = self.into_iter();
        let (lower, _) = iter.size_hint();
        // Pre-allocate with a reasonable guess (e.g., 10 chars per number + comma)
        let mut id_str = String::with_capacity(lower * 10);
        let mut empty = true;
        for (i, id) in iter.enumerate() {
            empty = false;
            if i > 0 {
                id_str.push(',');
            }
            write!(id_str, "{}", id)?;
        }
        if empty {
            bail!("Cannot create SQL IN clause from empty iterator");
        }
        Ok(id_str)
    }
}
