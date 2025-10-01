use crate::models::{PutioFile, PutioTransferResponse};
use serde::Deserialize;
use std::error::Error;

#[derive(Clone)]
pub struct PutioClient {
    token: String,
    base_url: String,
}

#[derive(Deserialize)]
struct FilesResponse {
    files: Vec<PutioFile>,
}

#[derive(Deserialize)]
struct AccountInfo {
    info: AccountData,
}

#[derive(Deserialize)]
struct AccountData {
    username: String,
}

impl PutioClient {
    pub fn new(token: String) -> Self {
        Self {
            token,
            base_url: "https://api.put.io/v2".to_string(),
        }
    }

    pub fn test_connection(&self) -> Result<String, Box<dyn Error>> {
        let response = ureq::get(&format!("{}/account/info", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.token))
            .call()?;

        let account: AccountInfo = serde_json::from_reader(response.into_reader())?;
        Ok(account.info.username)
    }

    pub fn find_or_create_folder(&self, folder_name: &str) -> Result<u64, Box<dyn Error>> {
        // List files in root (parent_id = 0)
        let response = ureq::get(&format!("{}/files/list?parent_id=0", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.token))
            .call()?;

        let files_response: FilesResponse = serde_json::from_reader(response.into_reader())?;

        // Check if folder exists
        for file in &files_response.files {
            if file.name == folder_name {
                return Ok(file.id);
            }
        }

        // Create folder if it doesn't exist
        let response = ureq::post(&format!("{}/files/create-folder", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.token))
            .send_form(&[("name", folder_name), ("parent_id", "0")])?;

        #[derive(Deserialize)]
        struct CreateFolderResponse {
            file: PutioFile,
        }

        let create_response: CreateFolderResponse = serde_json::from_reader(response.into_reader())?;
        Ok(create_response.file.id)
    }

    pub fn add_transfer(&self, magnet: &str, parent_id: u64) -> Result<u64, Box<dyn Error>> {
        let response = ureq::post(&format!("{}/transfers/add", self.base_url))
            .set("Authorization", &format!("Bearer {}", self.token))
            .send_form(&[
                ("url", magnet),
                ("save_parent_id", &parent_id.to_string()),
            ])?;

        let transfer_response: PutioTransferResponse = serde_json::from_reader(response.into_reader())?;
        Ok(transfer_response.transfer.id)
    }

    /// Initiate OAuth flow - returns authorization URL
    pub fn get_oauth_url(client_id: &str) -> String {
        format!(
            "https://app.put.io/v2/oauth2/authenticate?client_id={}&response_type=code&redirect_uri=urn:ietf:wg:oauth:2.0:oob",
            client_id
        )
    }

    /// Exchange OAuth code for access token
    pub fn exchange_code(
        client_id: &str,
        client_secret: &str,
        code: &str,
    ) -> Result<String, Box<dyn Error>> {
        let response = ureq::post("https://api.put.io/v2/oauth2/access_token")
            .send_form(&[
                ("client_id", client_id),
                ("client_secret", client_secret),
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", "urn:ietf:wg:oauth:2.0:oob"),
            ])?;

        #[derive(Deserialize)]
        struct TokenResponse {
            access_token: String,
        }

        let token_response: TokenResponse = serde_json::from_reader(response.into_reader())?;
        Ok(token_response.access_token)
    }
}