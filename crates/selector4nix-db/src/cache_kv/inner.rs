use std::sync::Arc;

use anyhow::{Context, Result as AnyhowResult};
use redb::{Database, ReadableDatabase, ReadableTableMetadata, TableDefinition, TableError};

pub type UnixTimestamp = u64;

type MainTableDefinition<'a> = TableDefinition<'a, &'static [u8], (u64, &'static [u8])>;
type ExpiryTableDefinition<'a> = TableDefinition<'a, (u64, &'static [u8]), ()>;

pub struct CacheKvInner {
    db: Arc<Database>,
    main_table: String,
    expiry_table: String,
}

impl CacheKvInner {
    pub fn new(db: Arc<Database>, entity_name: String) -> Self {
        assert!(!entity_name.is_empty());
        let expiry_table = format!("{entity_name}__expiry");
        Self {
            db,
            main_table: entity_name,
            expiry_table,
        }
    }

    pub fn get(
        &self,
        key: &[u8],
        now: UnixTimestamp,
    ) -> AnyhowResult<Option<(UnixTimestamp, Vec<u8>)>> {
        let transaction = self.db.begin_read()?;

        let main_table_def: MainTableDefinition<'_> = TableDefinition::new(&self.main_table);
        let main_table = transaction.open_table(main_table_def)?;

        let Some(value) = main_table.get(key)? else {
            return Ok(None);
        };

        if value.value().0 > now {
            Ok(Some((value.value().0, value.value().1.into())))
        } else {
            self.remove(key)
                .with_context(|| format!("could not cleanup expired entry of key `{key:?}`"))?;
            Ok(None)
        }
    }

    pub fn save(&self, key: &[u8], value: &[u8], expire_at: UnixTimestamp) -> AnyhowResult<()> {
        let transaction = self.db.begin_write()?;
        let main_table_def: MainTableDefinition<'_> = TableDefinition::new(&self.main_table);
        let expiry_table_def: ExpiryTableDefinition<'_> = TableDefinition::new(&self.expiry_table);

        {
            let mut main_table = transaction.open_table(main_table_def)?;
            let mut expiry_table = transaction.open_table(expiry_table_def)?;

            let new_value = (expire_at, value);
            if let Some(old_value) = main_table.insert(key, &new_value)? {
                let old_expiry_key = (old_value.value().0, key);
                expiry_table.remove(&old_expiry_key)?;
            }

            let new_expiry_key = (expire_at, key);
            expiry_table.insert(&new_expiry_key, &())?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn remove(&self, key: &[u8]) -> AnyhowResult<()> {
        let transaction = self.db.begin_write()?;
        let main_table_def: MainTableDefinition<'_> = TableDefinition::new(&self.main_table);
        let expiry_table_def: ExpiryTableDefinition<'_> = TableDefinition::new(&self.expiry_table);

        {
            let mut main_table = transaction.open_table(main_table_def)?;
            let mut expiry_table = transaction.open_table(expiry_table_def)?;

            let Some(value) = main_table.remove(key)? else {
                return Ok(());
            };

            let expiry_key = (value.value().0, key);
            expiry_table.remove(&expiry_key)?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn len(&self) -> AnyhowResult<usize> {
        let transaction = self.db.begin_read()?;
        let main_table_def: MainTableDefinition<'_> = TableDefinition::new(&self.main_table);
        match transaction.open_table(main_table_def) {
            Ok(table) => Ok(table.len()? as usize),
            Err(TableError::TableDoesNotExist(_)) => Ok(0),
            Err(err) => Err(err.into()),
        }
    }

    pub fn cleanup(&self, now: UnixTimestamp, limit: usize) -> AnyhowResult<usize> {
        let transaction = self.db.begin_write()?;
        let main_table_def: MainTableDefinition<'_> = TableDefinition::new(&self.main_table);
        let expiry_table_def: ExpiryTableDefinition<'_> = TableDefinition::new(&self.expiry_table);
        let mut expired_cnt = 0;

        {
            let mut main_table = transaction.open_table(main_table_def)?;
            let mut expiry_table = transaction.open_table(expiry_table_def)?;

            let upper_bound = (now + 1, [].as_slice());
            for item in expiry_table
                .extract_from_if(..upper_bound, |_, _| true)?
                .take(limit)
            {
                let item = item?;
                let expiry_key = item.0.value();
                main_table.remove(expiry_key.1)?;
                expired_cnt += 1;
            }
        }

        transaction.commit()?;
        Ok(expired_cnt)
    }
}
