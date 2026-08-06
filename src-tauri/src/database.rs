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
}