use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::error::Error;
use std::fs::{self, File};
use std::io::Read;
use tokio::sync::watch;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Serialize, Deserialize, Debug)]
struct Config {
    base_url: String,
    token: String,
    os_file_path: String,
}

#[async_trait]
trait NetworkDevice {
    async fn get_current_version(&self) -> Result<String, Box<dyn Error>>;
    async fn upload_os_file(&self, file_path: &str) -> Result<(), Box<dyn Error>>;
    async fn trigger_upgrade(&self) -> Result<(), Box<dyn Error>>;
    async fn check_upgrade_status(&self) -> Result<bool, Box<dyn Error>>;
}

struct VendorDevice {
    base_url: String,
    token: String,
}

#[async_trait]
impl NetworkDevice for VendorDevice {
    async fn get_current_version(&self) -> Result<String, Box<dyn Error>> {
        println!("Sending request to get current OS version...");
        let client = Client::new();
        let response = client
            .get(format!("{}/api/device/version", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await?;
        println!("Received response for OS version request.");
        let json_response = response.json::<Value>().await?;
        println!("Parsed response: {:?}", json_response);
        Ok(json_response["version"].as_str().unwrap_or("unknown").to_string())
    }

    async fn upload_os_file(&self, file_path: &str) -> Result<(), Box<dyn Error>> {
        println!("Opening OS file at path: {}", file_path);
        let mut file = File::open(file_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;
        println!("Read OS file successfully. File size: {} bytes", buffer.len());
        println!("Sending OS file to device...");
        let client = Client::new();
        let response = client
            .post(format!("{}/api/device/upload", self.base_url))
            .bearer_auth(&self.token)
            .body(buffer)
            .send()
            .await?;
        println!("OS file upload response status: {}", response.status());
        Ok(())
    }

    async fn trigger_upgrade(&self) -> Result<(), Box<dyn Error>> {
        println!("Sending request to trigger OS upgrade...");
        let client = Client::new();
        let response = client
            .post(format!("{}/api/device/upgrade", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await?;
        println!("Upgrade trigger response status: {}", response.status());
        Ok(())
    }

    async fn check_upgrade_status(&self) -> Result<bool, Box<dyn Error>> {
        println!("Checking upgrade status...");
        let client = Client::new();
        let response = client
            .get(format!("{}/api/device/upgrade/status", self.base_url))
            .bearer_auth(&self.token)
            .send()
            .await?;
        println!("Received status response: {}", response.status());
        let json_response = response.json::<Value>().await?;
        println!("Parsed upgrade status response: {:?}", json_response);
        Ok(json_response["status"] == "completed")
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Load the configuration file
    let config_path = "./config.json";
    println!("Loading configuration from {}", config_path);
    let config_data = fs::read_to_string(config_path)?;
    let config: Config = serde_json::from_str(&config_data)?;

    // Create a shared, mutable configuration for runtime updates
    let config = Arc::new(RwLock::new(config));

    // Watch for configuration updates (if needed, you can implement file watchers here)
    let (tx, mut rx) = watch::channel(config.clone());
    tokio::spawn(async move {
        while rx.changed().await.is_ok() {
            println!("Configuration updated: {:?}", *rx.borrow());
        }
    });

    // Step 1: Initialize the device with the configuration
    let device = VendorDevice {
        base_url: config.read().await.base_url.clone(),
        token: config.read().await.token.clone(),
    };

    // Step 2: Get Current OS Version
    println!("Checking current OS version...");
    let current_version = device.get_current_version().await?;
    println!("Current OS version: {}", current_version);

    // Step 3: Upload OS File
    println!("Uploading new OS file...");
    device.upload_os_file(&config.read().await.os_file_path).await?;
    println!("OS file uploaded successfully.");

    // Step 4: Trigger Upgrade
    println!("Triggering OS upgrade...");
    device.trigger_upgrade().await?;
    println!("OS upgrade initiated.");

    // Step 5: Monitor Upgrade Status
    println!("Monitoring upgrade status...");
    while !device.check_upgrade_status().await? {
        println!("Upgrade in progress...");
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
    }
    println!("Upgrade completed successfully.");

    // Step 6: Validate Upgrade
    let new_version = device.get_current_version().await?;
    println!("New OS version: {}", new_version);

    Ok(())
}
