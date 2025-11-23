#![allow(dead_code)]

use anyhow::Result;
use anyhow::anyhow;
use rand::Rng;
use reqwest::blocking::Client as ReqwestClient;
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const MAX_SEED: u64 = 1 << 63;
pub const HTTP_TIMEOUT_SECS: u64 = 5;

pub const PLACE: &str = "place";
pub const MOVE: &str = "move";
pub const PICKUP: &str = "pickup";
pub const DISCARD: &str = "discard";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub struct Action {
    pub timestamp: u64, // unix timestamp in microseconds
    pub id: String,
    pub action: String,
    pub target: String,
}

pub const HOT: &str = "hot";
pub const COLD: &str = "cold";
pub const ROOM: &str = "room";

pub const HEATER: &str = "heater";
pub const COOLER: &str = "cooler";
pub const SHELF: &str = "shelf";

#[derive(Debug, Clone, Deserialize)]
pub struct Order {
    pub id: String,
    pub name: String,
    pub temp: String,
    #[serde(default)]
    pub price: u64,
    pub freshness: u64, // in seconds
}

impl Action {
    pub fn new(id: &str, action_type: &str, target: &str, timestamp: SystemTime) -> Self {
        Self {
            action: action_type.to_string(),
            id: id.to_string(),
            target: target.to_string(),
            timestamp: timestamp
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_micros()
                .try_into()
                .unwrap(),
        }
    }
}

#[derive(Debug)]
pub struct Client {
    client: ReqwestClient,
    endpoint: String,
    auth: String,
}

impl Client {
    pub fn new(endpoint: &str, auth: &str) -> Self {
        Self {
            client: ReqwestClient::new(),
            endpoint: endpoint.to_string(),
            auth: auth.to_string(),
        }
    }

    pub fn challenge(&mut self, name: &str, seed: u64) -> Result<(Vec<Order>, String)> {
        let seed = (if seed == 0 {
            rand::rng().random_range(0..MAX_SEED)
        } else {
            seed
        })
        .to_string();

        let mut query_params: HashMap<&'static str, String> =
            HashMap::from([("seed", seed), ("auth", self.auth.clone())]);

        if !name.is_empty() {
            query_params.insert("name", name.to_string());
        }

        let url = reqwest::Url::parse_with_params(
            &format!("{}/interview/challenge/new", &self.endpoint),
            query_params.iter(),
        )?;

        let response = self
            .client
            .get(url.clone())
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .send()?;

        let test_id = response
            .headers()
            .get("x-test-id".to_string())
            .and_then(|v| v.to_str().ok().map(ToString::to_string))
            .unwrap_or_default();

        let orders = response.json()?;

        println!("Fetched new test problem, id={}: {}", test_id, url);
        Ok((orders, test_id))
    }

    pub fn solve(
        &mut self,
        test_id: &str,
        rate: Duration,
        min: Duration,
        max: Duration,
        actions: &[Action],
    ) -> Result<String> {
        let query = HashMap::from([("auth", &self.auth)]);

        let mut headers = HeaderMap::new();
        headers.insert("x-test-id", HeaderValue::from_str(test_id)?);
        headers.insert(CONTENT_TYPE, HeaderValue::from_str("application/json")?);

        let body = json!({
            "options": {
                "rate": rate.as_micros(),
                "min": min.as_micros(),
                "max": max.as_micros(),
            },
            "actions": actions
        });

        let response = self
            .client
            .post(format!("{}/interview/challenge/solve", &self.endpoint))
            .headers(headers)
            .query(&query)
            .json(&body)
            .timeout(Duration::from_secs(HTTP_TIMEOUT_SECS))
            .send()?;

        response
            .text()
            .map_err(|_| anyhow!("failed to validate solution"))
    }
}
