use sqlx::MySqlPool;
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
pub struct Cape {
    pub id: String,
    pub name: String,
    pub description: String,
    pub price: i64,
    pub purchasable: bool,
    pub texture_filename: String,
    pub owned: bool,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CosmeticsProfile {
    pub balance: i64,
    pub selected_cape_id: Option<String>,
    pub capes: Vec<Cape>,
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
                source ENUM('purchase', 'grant') NOT NULL DEFAULT 'grant',
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

    pub async fn get_cosmetics_profile(&self, discord_id: &str) -> Result<CosmeticsProfile, String> {
        sqlx::query("INSERT IGNORE INTO wallets (discord_id, balance) VALUES (?, 0)")
            .bind(discord_id)
            .execute(&self.pool)
            .await
            .map_err(|error| error.to_string())?;

        let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE discord_id = ?")
            .bind(discord_id)
            .fetch_one(&self.pool)
            .await
            .map_err(|error| error.to_string())?;

        let rows = sqlx::query_as::<_, (String, String, Option<String>, i64, i8, String, i8, i8)>(
            r#"
            SELECT c.id, c.name, c.description, c.price, c.purchasable, c.texture_filename,
                   CASE WHEN uc.cape_id IS NULL THEN 0 ELSE 1 END AS owned,
                   CASE WHEN pc.cape_id = c.id THEN 1 ELSE 0 END AS selected
            FROM capes c
            LEFT JOIN user_capes uc ON uc.cape_id = c.id AND uc.discord_id = ?
            LEFT JOIN player_capes pc ON pc.discord_id = ?
            WHERE c.enabled = 1 AND (c.purchasable = 1 OR uc.cape_id IS NOT NULL)
            ORDER BY c.price ASC, c.name ASC
            "#,
        )
        .bind(discord_id)
        .bind(discord_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|error| error.to_string())?;

        let capes: Vec<Cape> = rows
            .into_iter()
            .map(|(id, name, description, price, purchasable, texture_filename, owned, selected)| Cape {
                id,
                name,
                description: description.unwrap_or_default(),
                price,
                purchasable: purchasable != 0,
                texture_filename,
                owned: owned != 0,
                selected: selected != 0,
            })
            .collect();
        let selected_cape_id = capes.iter().find(|cape| cape.selected).map(|cape| cape.id.clone());

        Ok(CosmeticsProfile {
            balance,
            selected_cape_id,
            capes,
        })
    }

    pub async fn purchase_cape(&self, discord_id: &str, cape_id: &str) -> Result<CosmeticsProfile, String> {
        let mut transaction = self.pool.begin().await.map_err(|error| error.to_string())?;

        let result = async {
            sqlx::query("INSERT IGNORE INTO wallets (discord_id, balance) VALUES (?, 0)")
                .bind(discord_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| error.to_string())?;

            let balance: i64 = sqlx::query_scalar("SELECT balance FROM wallets WHERE discord_id = ? FOR UPDATE")
                .bind(discord_id)
                .fetch_one(&mut *transaction)
                .await
                .map_err(|error| error.to_string())?;
            let cape = sqlx::query_as::<_, (i64, i8, i8)>(
                "SELECT price, purchasable, enabled FROM capes WHERE id = ? FOR UPDATE",
            )
            .bind(cape_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?
            .ok_or_else(|| "Cette cape n'existe plus.".to_string())?;

            if cape.2 == 0 || cape.1 == 0 {
                return Err("Cette cape n'est pas disponible à l'achat.".to_string());
            }
            let already_owned: Option<i8> = sqlx::query_scalar(
                "SELECT 1 FROM user_capes WHERE discord_id = ? AND cape_id = ?",
            )
            .bind(discord_id)
            .bind(cape_id)
            .fetch_optional(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?;
            if already_owned.is_some() {
                return Err("Tu possèdes déjà cette cape.".to_string());
            }
            if balance < cape.0 {
                return Err("Ton solde est insuffisant pour cette cape.".to_string());
            }

            let next_balance = balance - cape.0;
            sqlx::query("UPDATE wallets SET balance = ? WHERE discord_id = ?")
                .bind(next_balance)
                .bind(discord_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| error.to_string())?;
            sqlx::query("INSERT INTO user_capes (discord_id, cape_id, source) VALUES (?, ?, 'purchase')")
                .bind(discord_id)
                .bind(cape_id)
                .execute(&mut *transaction)
                .await
                .map_err(|error| error.to_string())?;
            sqlx::query(
                "INSERT INTO wallet_transactions (discord_id, amount, balance_after, reason, reference_id) VALUES (?, ?, ?, 'purchase', ?)",
            )
            .bind(discord_id)
            .bind(-cape.0)
            .bind(next_balance)
            .bind(cape_id)
            .execute(&mut *transaction)
            .await
            .map_err(|error| error.to_string())?;

            Ok::<(), String>(())
        }
        .await;

        match result {
            Ok(()) => transaction.commit().await.map_err(|error| error.to_string())?,
            Err(error) => {
                transaction.rollback().await.map_err(|rollback_error| rollback_error.to_string())?;
                return Err(error);
            }
        }

        self.get_cosmetics_profile(discord_id).await
    }

    pub async fn select_cape(
        &self,
        discord_id: &str,
        cape_id: Option<&str>,
    ) -> Result<CosmeticsProfile, String> {
        if let Some(cape_id) = cape_id {
            let owned: Option<i8> = sqlx::query_scalar(
                r#"
                SELECT 1 FROM user_capes uc
                INNER JOIN capes c ON c.id = uc.cape_id
                WHERE uc.discord_id = ? AND uc.cape_id = ? AND c.enabled = 1
                "#,
            )
            .bind(discord_id)
            .bind(cape_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|error| error.to_string())?;
            if owned.is_none() {
                return Err("Cette cape n'est pas disponible dans ta collection.".to_string());
            }
            sqlx::query(
                "INSERT INTO player_capes (discord_id, cape_id) VALUES (?, ?) ON DUPLICATE KEY UPDATE cape_id = VALUES(cape_id)",
            )
            .bind(discord_id)
            .bind(cape_id)
            .execute(&self.pool)
            .await
            .map_err(|error| error.to_string())?;
        } else {
            sqlx::query("DELETE FROM player_capes WHERE discord_id = ?")
                .bind(discord_id)
                .execute(&self.pool)
                .await
                .map_err(|error| error.to_string())?;
        }

        self.get_cosmetics_profile(discord_id).await
    }
}
