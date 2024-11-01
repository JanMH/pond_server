use std::collections::HashMap;

use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};

#[cfg(test)]
use mockall::{automock, predicate::*};

#[allow(unused)]
pub struct CloudflareClient {
    cloudflare_base_url: String,
    api_key: String,
    api_client: reqwest::blocking::Client,
}

#[allow(unused)]
#[cfg_attr(test, automock)]
impl CloudflareClient {
    pub fn new(api_key: String) -> Self {
        let mut default_headers = HeaderMap::new();
        default_headers.insert(
            "Authorization",
            HeaderValue::from_str(&format!("Bearer {}", api_key)).unwrap(),
        );

        let api_client = reqwest::blocking::Client::builder()
            .default_headers(default_headers)
            .build()
            .unwrap();

        CloudflareClient {
            cloudflare_base_url: "https://api.cloudflare.com".to_string(),
            api_key,
            api_client,
        }
    }

    pub fn list_zones(&self, name: &str) -> anyhow::Result<CloudflareListZonesResponse> {
        let url = format!("{}/client/v4/zones", self.cloudflare_base_url);
        let response = self
            .api_client
            .get(&url)
            .query(&[("name", name)])
            .send()
            .unwrap();
        let response = serde_json::from_reader(response)?;
        Ok(response)
    }

    pub fn add_dns_record(
        &self,
        zone_id: &ZoneId,
        request: &CloudflareDnsRecordBody,
    ) -> anyhow::Result<CloudflareCreateRecordResponse> {
        let url = format!(
            "{}/client/v4/zones/{}/dns_records",
            self.cloudflare_base_url, zone_id.0
        );

        let response = self.api_client.post(&url).json(request).send()?;
        let response = serde_json::from_reader(response)?;
        Ok(response)
    }

    pub fn update_dns_record(
        &self,
        zone_id: &ZoneId,
        record_id: &RecordId,
        request: &CloudflareDnsRecordBody,
    ) -> anyhow::Result<CloudflareUpdateRecordResponse> {
        let url = format!(
            "{}/client/v4/zones/{}/dns_records/{}",
            self.cloudflare_base_url, zone_id.0, record_id.0
        );

        let response = self.api_client.patch(&url).json(request).send()?;
        let response = serde_json::from_reader(response)?;
        Ok(response)
    }

    pub fn list_dns_records(
        &self,
        zone_id: &ZoneId,
        domain_name: &str,
        page: u32,
    ) -> anyhow::Result<CloudflareListRecordsResponse> {
        let url = format!(
            "{}/client/v4/zones/{}/dns_records",
            self.cloudflare_base_url, zone_id.0
        );

        let page = page.to_string();
        let query_params = [("page", page.as_str()), ("name", domain_name)];

        let response = self.api_client.get(&url).query(&query_params).send()?;
        let response = serde_json::from_reader(response)?;

        Ok(response)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct ZoneId(pub String);

#[derive(Clone, Debug, Deserialize)]
pub struct RecordId(pub String);

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct CloudflareEnvelope<T> {
    pub result: T,
    pub success: bool,
    pub errors: Vec<CloudflareMessage>,
    pub messages: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct CloudflareV4Result<T> {
    pub errors: Vec<CloudflareMessage>,
    pub messages: Vec<CloudflareMessage>,
    pub success: bool,
    pub result: Option<T>,
    pub result_info: Option<CloudflareV4ResultInfo>,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct CloudflareMessage {
    pub code: u32,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct CloudflareV4ResultInfo {
    pub page: u32,
    pub per_page: u32,
    pub count: u32,
    pub total_count: u32,
}

pub type CloudflareListRecordsResponse = CloudflareV4Result<ResultOrObject<Vec<GetDnsRecord>>>;

pub type CloudflareListZonesResponse = CloudflareV4Result<ResultOrObject<Vec<Zone>>>;

pub type CloudflareCreateRecordResponse = CloudflareV4Result<ResultOrObject<GetDnsRecord>>;

pub type CloudflareUpdateRecordResponse = CloudflareV4Result<Option<GetDnsRecord>>;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
#[allow(unused)]
pub enum ResultOrObject<T> {
    Result(T),
    Object(HashMap<String, String>),
}

#[allow(unused)]
impl<T> ResultOrObject<T> {
    pub fn is_result(&self) -> bool {
        matches!(self, ResultOrObject::Result(_))
    }

    pub fn unwrap(self) -> T {
        match self {
            ResultOrObject::Result(value) => value,
            ResultOrObject::Object(_) => {
                panic!("called `ResultOrObject::unwrap()` on an `Object` value")
            }
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[allow(unused)]
pub struct Zone {
    pub id: ZoneId,
    pub name: String,
    pub status: String,
    pub paused: bool,
    #[serde(rename = "type")]
    pub type_: String,
    pub development_mode: u32,
    pub name_servers: Vec<String>,
    pub original_name_servers: Vec<String>,
    pub original_registrar: Option<String>,
    pub original_dnshost: Option<String>,
    pub modified_on: String,
    pub created_on: String,
    pub activated_on: String,
}

#[derive(Clone, Debug, Deserialize)]
#[allow(unused)]
pub struct GetDnsRecord {
    pub id: RecordId,
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
}

#[derive(Debug, Serialize)]
#[allow(unused)]
pub struct CloudflareDnsRecordBody {
    #[serde(rename = "type")]
    pub type_: String,
    pub name: String,
    pub comment: Option<String>,
    pub content: String,
    pub ttl: u32,
    pub proxied: bool,
}

#[cfg(test)]
pub mod testhelpers {
    pub const ADD_RECORD_FAILURE_RESPONSE: &str = r#"{
              "errors": [
                {
                  "code": 1004,
                  "message": "DNS Validation Error"
                }
              ],
              "messages": [],
              "success": false,
              "result": null
            }"#;
    pub const LIST_ZONES_RESPONSE: &str = r#"{
      "errors": [],
      "messages": [],
      "success": true,
      "result_info": {
        "count": 1,
        "page": 1,
        "per_page": 20,
        "total_count": 2000
      },
      "result": [
        {
          "account": {
            "id": "023e105f4ecef8ad9ca31a8372d0c353",
            "name": "Example Account Name"
          },
          "activated_on": "2014-01-02T00:01:00.12345Z",
          "created_on": "2014-01-01T05:20:00.12345Z",
          "development_mode": 7200,
          "id": "023e105f4ecef8ad9ca31a8372d0c353",
          "meta": {
            "cdn_only": true,
            "custom_certificate_quota": 1,
            "dns_only": true,
            "foundation_dns": true,
            "page_rule_quota": 100,
            "phishing_detected": false,
            "step": 2
          },
          "modified_on": "2014-01-01T05:20:00.12345Z",
          "name": "example.com",
          "name_servers": [
            "bob.ns.cloudflare.com",
            "lola.ns.cloudflare.com"
          ],
          "original_dnshost": "NameCheap",
          "original_name_servers": [
            "ns1.originaldnshost.com",
            "ns2.originaldnshost.com"
          ],
          "original_registrar": "GoDaddy",
          "owner": {
            "id": "023e105f4ecef8ad9ca31a8372d0c353",
            "name": "Example Org",
            "type": "organization"
          },
          "paused": false,
          "status": "active",
          "type": "full",
          "vanity_name_servers": [
            "ns1.example.com",
            "ns2.example.com"
          ]
        }
      ]
    }"#;

    pub const LIST_RECORDS_RESPONSE: &str = r#"{
      "errors": [],
      "messages": [],
      "success": true,
      "result_info": {
        "count": 1,
        "page": 1,
        "per_page": 20,
        "total_count": 2000
      },
      "result": [
        {
          "comment": "Domain verification record",
          "name": "example.com",
          "proxied": true,
          "settings": {},
          "tags": [],
          "ttl": 3600,
          "content": "198.51.100.4",
          "type": "A",
          "comment_modified_on": "2024-01-01T05:20:00.12345Z",
          "created_on": "2014-01-01T05:20:00.12345Z",
          "id": "023e105f4ecef8ad9ca31a8372d0c353",
          "meta": {},
          "modified_on": "2014-01-01T05:20:00.12345Z",
          "proxiable": true,
          "tags_modified_on": "2025-01-01T05:20:00.12345Z"
        }
      ]
    }"#;

    pub const ADD_RECORD_RESPONSE: &str = r#"{
        "errors": [],
        "messages": [],
        "success": true,
        "result": {
        "comment": "Domain verification record",
        "name": "example.com",
        "proxied": true,
        "settings": {},
        "tags": [],
        "ttl": 3600,
        "content": "198.51.100.4",
        "type": "A",
        "comment_modified_on": "2024-01-01T05:20:00.12345Z",
        "created_on": "2014-01-01T05:20:00.12345Z",
        "id": "023e105f4ecef8ad9ca31a8372d0c353",
        "meta": {},
        "modified_on": "2014-01-01T05:20:00.12345Z",
        "proxiable": true,
        "tags_modified_on": "2025-01-01T05:20:00.12345Z"
        }
    }"#;
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::{Matcher, Server, ServerGuard};
    use testhelpers::ADD_RECORD_FAILURE_RESPONSE;

    fn test_client(server: &ServerGuard) -> CloudflareClient {
        let mut client = CloudflareClient::new("test_api_key".to_string());
        client.cloudflare_base_url = server.url();
        client
    }

    #[test]
    fn test_list_zones() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/client/v4/zones?name=example.com")
            .with_status(200)
            .with_body(testhelpers::LIST_ZONES_RESPONSE)
            .create();

        let client = test_client(&server);
        let response = client.list_zones("example.com").unwrap();
        assert!(response.result.is_some());
        assert!(response.result.unwrap().is_result());
        assert!(response.success);
    }

    #[test]
    fn test_list_zones_404() {
        let mut server = Server::new();
        let _m = server
            .mock("GET", "/client/v4/zones?name=example.com")
            .with_status(404)
            .with_body(
                r#"{
                      "errors": [],
                      "messages": [],
                      "result": {},
                      "success": false
                    }"#,
            )
            .create();

        let client = test_client(&server);
        let response = client.list_zones("example.com").unwrap();
        assert!(response.result.is_some());
        assert!(!response.result.unwrap().is_result());
        assert!(!response.success);
    }

    #[test]
    fn test_list_dns_records() {
        let zone_id = ZoneId("test_zone_id".to_string());
        let domain_name = "example.com";
        let page = 1;

        let mut server = Server::new();
        let _m = server
            .mock("GET", "/client/v4/zones/test_zone_id/dns_records")
            .match_header("Authorization", "Bearer test_api_key")
            .match_query(Matcher::AllOf(vec![
                Matcher::UrlEncoded("page".into(), "1".into()),
                Matcher::UrlEncoded("name".into(), domain_name.into()),
            ]))
            .with_status(200)
            .with_body(testhelpers::LIST_RECORDS_RESPONSE)
            .create();

        let client = test_client(&server);
        let response = client
            .list_dns_records(&zone_id, domain_name, page)
            .unwrap();
        assert!(response.result.is_some());
    }

    #[test]
    fn test_add_dns_record() {
        let zone_id = ZoneId("test_zone_id".to_string());
        let request = CloudflareDnsRecordBody {
            type_: "A".to_string(),
            name: "example.com".to_string(),
            comment: Some("Test record".to_string()),
            content: "198.51.100.4".to_string(),
            ttl: 3600,
            proxied: true,
        };

        let mut server = Server::new();

        let _m = server
            .mock("POST", "/client/v4/zones/test_zone_id/dns_records")
            .match_header("Authorization", "Bearer test_api_key")
            .match_body(Matcher::JsonString(
                serde_json::to_string(&request).unwrap(),
            ))
            .with_status(200)
            .with_body(testhelpers::ADD_RECORD_RESPONSE)
            .create();

        let client = test_client(&server);
        let response = client.add_dns_record(&zone_id, &request).unwrap();
        assert!(response.result.is_some());
        let record = response.result.unwrap().unwrap();
        assert_eq!(record.id.0, "023e105f4ecef8ad9ca31a8372d0c353");
        assert_eq!(record.type_, "A");
        assert_eq!(record.name, "example.com");
        assert_eq!(record.content, "198.51.100.4");
        assert_eq!(record.ttl, 3600);
        assert!(record.proxied);
    }

    #[test]
    fn test_add_dns_record_error() {
        let zone_id = ZoneId("test_zone_id".to_string());
        let request = CloudflareDnsRecordBody {
            type_: "A".to_string(),
            name: "example.com".to_string(),
            comment: Some("Test record".to_string()),
            content: "198.51.100.4".to_string(),
            ttl: 3600,
            proxied: true,
        };

        let mut server = Server::new();
        let _m = server
            .mock("POST", "/client/v4/zones/test_zone_id/dns_records")
            .match_header("Authorization", "Bearer test_api_key")
            .match_body(Matcher::JsonString(
                serde_json::to_string(&request).unwrap(),
            ))
            .with_status(400)
            .with_body(ADD_RECORD_FAILURE_RESPONSE)
            .create();

        let client = test_client(&server);
        let response = client.add_dns_record(&zone_id, &request);
        dbg!(&response);
        let response = response.unwrap();
        assert!(!response.success);
    }

    #[test]
    fn test_update_dns_record() {
        let zone_id = ZoneId("test_zone_id".to_string());
        let record_id = RecordId("test_record_id".to_string());
        let request = CloudflareDnsRecordBody {
            type_: "A".to_string(),
            name: "example.com".to_string(),
            comment: Some("Updated record".to_string()),
            content: "198.51.100.5".to_string(),
            ttl: 3600,
            proxied: true,
        };

        let mut server = Server::new();
        let _m = server
            .mock(
                "PATCH",
                "/client/v4/zones/test_zone_id/dns_records/test_record_id",
            )
            .match_header("Authorization", "Bearer test_api_key")
            .match_body(Matcher::JsonString(
                serde_json::to_string(&request).unwrap(),
            ))
            .with_status(200)
            .with_body(
                r#"{
                  "errors": [],
                  "messages": [],
                  "success": true,
                  "result": {
                    "comment": "Domain verification record",
                    "name": "example.com",
                    "proxied": true,
                    "settings": {},
                    "tags": [],
                    "ttl": 3600,
                    "content": "198.51.100.4",
                    "type": "A",
                    "comment_modified_on": "2024-01-01T05:20:00.12345Z",
                    "created_on": "2014-01-01T05:20:00.12345Z",
                    "id": "023e105f4ecef8ad9ca31a8372d0c353",
                    "meta": {},
                    "modified_on": "2014-01-01T05:20:00.12345Z",
                    "proxiable": true,
                    "tags_modified_on": "2025-01-01T05:20:00.12345Z"
                  }
                }"#,
            )
            .create();

        let client = test_client(&server);
        let response = client
            .update_dns_record(&zone_id, &record_id, &request)
            .unwrap();
        assert!(response.success);
    }

    #[test]
    fn test_update_dns_record_error() {
        let zone_id = ZoneId("test_zone_id".to_string());
        let record_id = RecordId("test_record_id".to_string());
        let request = CloudflareDnsRecordBody {
            type_: "A".to_string(),
            name: "example.com".to_string(),
            comment: Some("Updated record".to_string()),
            content: "198.51.100.5".to_string(),
            ttl: 3600,
            proxied: true,
        };

        let mut server = Server::new();
        let _m = server
            .mock(
                "PATCH",
                "/client/v4/zones/test_zone_id/dns_records/test_record_id",
            )
            .match_header("Authorization", "Bearer test_api_key")
            .match_body(Matcher::JsonString(
                serde_json::to_string(&request).unwrap(),
            ))
            .with_status(400)
            .with_body(
                r#"{
                  "errors": [
                    {
                      "code": 1004,
                      "message": "DNS Validation Error"
                    }
                  ],
                  "messages": [],
                  "success": false,
                  "result": null
                }"#,
            )
            .create();

        let client = test_client(&server);
        let response = client
            .update_dns_record(&zone_id, &record_id, &request)
            .unwrap();
        assert!(!response.success);
    }
}
