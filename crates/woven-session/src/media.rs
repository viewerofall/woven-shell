use anyhow::Result;
use tokio::process::Command;

pub struct MediaReader;

impl MediaReader {
    pub async fn read() -> Result<(bool, String, String)> {
        let status = Self::run_playerctl("status").await.ok();
        let title = Self::run_playerctl("metadata --format {{ title }}").await.unwrap_or_default();
        let artist = Self::run_playerctl("metadata --format {{ artist }}").await.unwrap_or_default();

        let playing = status.as_deref() == Some("Playing");
        Ok((playing, title, artist))
    }

    async fn run_playerctl(args: &str) -> Result<String> {
        let output = Command::new("playerctl")
            .args(args.split_whitespace())
            .output()
            .await?;

        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
        } else {
            anyhow::bail!("playerctl failed")
        }
    }
}
