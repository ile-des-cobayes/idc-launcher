use sqlx::{MySqlPool, Row};
use serde::Serialize;

pub struct Database {
    pool: MySqlPool,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct User {
    pub id: i64,
    pub discord_id: String,
    pub username: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PlayerSkin {
    pub discord_id: String,
    pub model: String,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapeShopItem {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub price: u64,
    pub purchasable: bool,
    pub owned: bool,
    pub selected: bool,
    pub texture_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapeShopProfile {
    pub balance: u64,
    pub selected_cape_id: Option<String>,
    pub capes: Vec<CapeShopItem>,
}

impl Database {
    pub async fn new() -> Result<Self, sqlx::Error> {
        let database_url = format!(
            "mysql://{}:{}@{}:{}/{}",
            env!("MYSQL_USER"),
            env!("MYSQL_PASSWORD"),
            env!("MYSQL_HOST"),
            option_env!("MYSQL_PORT").unwrap_or("19855"),
            env!("MYSQL_DATABASE")
        );

        let pool = MySqlPool::connect(&database_url).await?;

        Ok(Self { pool })
    }

    pub async fn create_tables(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS users (
                id INT AUTO_INCREMENT PRIMARY KEY,
                discord_id VARCHAR(255) UNIQUE NOT NULL,
                username VARCHAR(255) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS player_skins (
                discord_id VARCHAR(255) PRIMARY KEY,
                model ENUM('default', 'slim') NOT NULL DEFAULT 'default',
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            "#,
        )
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS wallets (
                discord_id VARCHAR(255) PRIMARY KEY,
                balance BIGINT UNSIGNED NOT NULL DEFAULT 0,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            "#,
        )
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS capes (
                id VARCHAR(80) PRIMARY KEY,
                name VARCHAR(80) NOT NULL,
                description TEXT NULL,
                price BIGINT UNSIGNED NOT NULL DEFAULT 0,
                purchasable TINYINT(1) NOT NULL DEFAULT 1,
                enabled TINYINT(1) NOT NULL DEFAULT 1,
                texture_filename VARCHAR(180) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            "#,
        )
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS user_capes (
                discord_id VARCHAR(255) NOT NULL,
                cape_id VARCHAR(80) NOT NULL,
                source ENUM('purchase','grant') NOT NULL DEFAULT 'grant',
                acquired_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                PRIMARY KEY (discord_id, cape_id),
                INDEX user_capes_cape_id_idx (cape_id)
            )
            "#,
        )
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS player_capes (
                discord_id VARCHAR(255) PRIMARY KEY,
                cape_id VARCHAR(80) NULL,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP ON UPDATE CURRENT_TIMESTAMP
            )
            "#,
        )
            .execute(&self.pool)
            .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS wallet_transactions (
                id BIGINT AUTO_INCREMENT PRIMARY KEY,
                discord_id VARCHAR(255) NOT NULL,
                amount BIGINT NOT NULL,
                balance_after BIGINT UNSIGNED NOT NULL,
                reason VARCHAR(32) NOT NULL,
                reference_id VARCHAR(80) NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                INDEX wallet_transactions_user_idx (discord_id, created_at)
            )
            "#,
        )
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_user_by_discord_id(&self, discord_id: &str) -> Result<Option<User>, sqlx::Error> {
        let row = sqlx::query_as::<_, (i64, String, String, chrono::DateTime<chrono::Utc>)>(
            "SELECT id, discord_id, username, created_at FROM users WHERE discord_id = ?",
        )
            .bind(discord_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|(id, discord_id, username, created_at)| User {
            id,
            discord_id,
            username,
            created_at,
        }))
    }

    pub async fn create_user(&self, discord_id: &str, username: &str) -> Result<User, sqlx::Error> {
        let result = sqlx::query("INSERT INTO users (discord_id, username) VALUES (?, ?)")
            .bind(discord_id)
            .bind(username)
            .execute(&self.pool)
            .await?;

        let id = result.last_insert_id() as i64;

        Ok(User {
            id,
            discord_id: discord_id.to_string(),
            username: username.to_string(),
            created_at: chrono::Utc::now(),
        })
    }

    pub async fn update_username(&self, discord_id: &str, username: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE users SET username = ? WHERE discord_id = ?")
            .bind(username)
            .bind(discord_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_skin_model(&self, discord_id: &str) -> Result<Option<String>, sqlx::Error> {
        let row = sqlx::query_as::<_, (String,)>(
            "SELECT model FROM player_skins WHERE discord_id = ?",
        )
            .bind(discord_id)
            .fetch_optional(&self.pool)
            .await?;

        Ok(row.map(|(model,)| model))
    }

    pub async fn update_skin_model(&self, discord_id: &str, model: &str) -> Result<(), sqlx::Error> {
        sqlx::query("INSERT INTO player_skins (discord_id, model) VALUES (?, ?) ON DUPLICATE KEY UPDATE model = ?")
            .bind(discord_id)
            .bind(model)
            .bind(model)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_cape_shop_profile(
        &self,
        discord_id: &str,
        skin_api_url: &str,
    ) -> Result<CapeShopProfile, String> {
        sqlx::query("INSERT IGNORE INTO wallets (discord_id, balance) VALUES (?, 0)")
            .bind(discord_id)
            .execute(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let balance_row = sqlx::query("SELECT balance FROM wallets WHERE discord_id = ?")
            .bind(discord_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| e.to_string())?;
        let balance = balance_row.try_get::<u64, _>("balance").map_err(|e| e.to_string())?;

        let selected_cape_id = sqlx::query("SELECT cape_id FROM player_capes WHERE discord_id = ?")
            .bind(discord_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| e.to_string())?
            .and_then(|row| row.try_get::<Option<String>, _>("cape_id").ok().flatten());

        let rows = sqlx::query(
            r#"
            SELECT
                c.id,
                c.name,
                c.description,
                c.price,
                c.purchasable,
                c.texture_filename,
                CASE WHEN uc.discord_id IS NULL THEN 0 ELSE 1 END AS owned
            FROM capes c
            LEFT JOIN user_capes uc
                ON uc.cape_id = c.id
               AND uc.discord_id = ?
            WHERE c.enabled = 1
            ORDER BY c.price ASC, c.name ASC
            "#,
        )
            .bind(discord_id)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| e.to_string())?;

        let base = skin_api_url.trim_end_matches('/');
        let mut capes = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.try_get("id").map_err(|e| e.to_string())?;
            let texture_filename: String = row.try_get("texture_filename").map_err(|e| e.to_string())?;
            capes.push(CapeShopItem {
                selected: selected_cape_id.as_deref() == Some(id.as_str()),
                id,
                name: row.try_get("name").map_err(|e| e.to_string())?,
                description: row.try_get("description").map_err(|e| e.to_string())?,
                price: row.try_get("price").map_err(|e| e.to_string())?,
                purchasable: row.try_get::<i8, _>("purchasable").map_err(|e| e.to_string())? == 1,
                owned: row.try_get::<i8, _>("owned").map_err(|e| e.to_string())? == 1,
                texture_url: format!("{base}/textures/{texture_filename}"),
            });
        }

        let visible_selected = selected_cape_id
            .and_then(|id| capes.iter().any(|cape| cape.id == id).then_some(id));

        Ok(CapeShopProfile {
            balance,
            selected_cape_id: visible_selected,
            capes,
        })
    }

    pub async fn purchase_cape(
        &self,
        discord_id: &str,
        cape_id: &str,
        skin_api_url: &str,
    ) -> Result<CapeShopProfile, String> {
        let mut tx = self.pool.begin().await.map_err(|e| e.to_string())?;

        let cape_row = sqlx::query(
            "SELECT id, price, purchasable, enabled FROM capes WHERE id = ? LIMIT 1",
        )
            .bind(cape_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?;

        let Some(cape_row) = cape_row else {
            return Err("Cette cape n'existe pas.".to_string());
        };

        let enabled = cape_row.try_get::<i8, _>("enabled").map_err(|e| e.to_string())? == 1;
        if !enabled {
            return Err("Cette cape n'est plus disponible.".to_string());
        }

        let purchasable = cape_row.try_get::<i8, _>("purchasable").map_err(|e| e.to_string())? == 1;
        if !purchasable {
            return Err("Cette cape ne peut pas être achetée.".to_string());
        }

        let already_owned = sqlx::query(
            "SELECT 1 FROM user_capes WHERE discord_id = ? AND cape_id = ? LIMIT 1",
        )
            .bind(discord_id)
            .bind(cape_id)
            .fetch_optional(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?
            .is_some();

        if already_owned {
            return Err("Tu possèdes déjà cette cape.".to_string());
        }

        let price: u64 = cape_row.try_get("price").map_err(|e| e.to_string())?;
        sqlx::query("INSERT IGNORE INTO wallets (discord_id, balance) VALUES (?, 0)")
            .bind(discord_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?;

        let wallet_row = sqlx::query("SELECT balance FROM wallets WHERE discord_id = ? FOR UPDATE")
            .bind(discord_id)
            .fetch_one(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?;
        let balance: u64 = wallet_row.try_get("balance").map_err(|e| e.to_string())?;

        if balance < price {
            return Err(format!(
                "Solde insuffisant : il te faut {} éclats de plus.",
                price - balance
            ));
        }
        

        let next_balance = balance - price;
        sqlx::query("UPDATE wallets SET balance = ? WHERE discord_id = ?")
            .bind(next_balance)
            .bind(discord_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query("INSERT INTO user_capes (discord_id, cape_id, source) VALUES (?, ?, 'purchase')")
            .bind(discord_id)
            .bind(cape_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?;

        sqlx::query(
            "INSERT INTO player_capes (discord_id, cape_id) VALUES (?, ?) ON DUPLICATE KEY UPDATE cape_id = VALUES(cape_id)",
        )
            .bind(discord_id)
            .bind(cape_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?;

        let debit = i64::try_from(price)
            .map_err(|_| "Prix de cape invalide (hors limites supportées).".to_string())?;
        sqlx::query(
            "INSERT INTO wallet_transactions (discord_id, amount, balance_after, reason, reference_id) VALUES (?, ?, ?, 'cape_purchase', ?)",
        )
            .bind(discord_id)
            .bind(-debit)
            .bind(next_balance)
            .bind(cape_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| e.to_string())?;

        tx.commit().await.map_err(|e| e.to_string())?;
        self.get_cape_shop_profile(discord_id, skin_api_url).await
    }

    pub async fn select_cape(
        &self,
        discord_id: &str,
        cape_id: Option<&str>,
        skin_api_url: &str,
    ) -> Result<CapeShopProfile, String> {
        if let Some(cape_id) = cape_id {
            let can_select = sqlx::query(
                r#"
                SELECT 1
                FROM user_capes uc
                INNER JOIN capes c ON c.id = uc.cape_id
                WHERE uc.discord_id = ?
                  AND uc.cape_id = ?
                  AND c.enabled = 1
                LIMIT 1
                "#,
            )
                .bind(discord_id)
                .bind(cape_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| e.to_string())?
                .is_some();

            if !can_select {
                return Err("Cette cape ne fait pas partie de ta collection active.".to_string());
            }

            sqlx::query(
                "INSERT INTO player_capes (discord_id, cape_id) VALUES (?, ?) ON DUPLICATE KEY UPDATE cape_id = VALUES(cape_id)",
            )
                .bind(discord_id)
                .bind(cape_id)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            sqlx::query(
                "INSERT INTO player_capes (discord_id, cape_id) VALUES (?, NULL) ON DUPLICATE KEY UPDATE cape_id = NULL",
            )
                .bind(discord_id)
                .execute(&self.pool)
                .await
                .map_err(|e| e.to_string())?;
        }

        self.get_cape_shop_profile(discord_id, skin_api_url).await
    }
}