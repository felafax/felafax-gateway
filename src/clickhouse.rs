use clickhouse::Client;

pub struct Clickhouse {
    client: Client,
}

impl Clickhouse {
    pub fn new(url: &str, username: &str, pwd: &str, database: &str) -> Self {
        Self {
            client: Client::default()
                .with_url(url)
                .with_user(username)
                .with_password(pwd),
        }
    }

    pub async fn insert_batch<T>(
        &self,
        table: &str,
        data: Vec<T>,
    ) -> Result<(), clickhouse::error::Error>
    where
        T: serde::Serialize + clickhouse::Row,
    {
        let mut insert = self.client.insert(table)?;
        for row in data {
            insert.write(&row).await?;
        }
        insert.end().await?;
        Ok(())
    }

    pub async fn insert_row<T>(&self, table: &str, row: T) -> Result<(), clickhouse::error::Error>
    where
        T: serde::Serialize + clickhouse::Row,
    {
        let mut insert = self.client.insert(table)?;
        insert.write(&row).await?;
        insert.end().await?;
        Ok(())
    }
}
