use anyhow::anyhow;
use client::{CloudflareDnsRecordBody, GetDnsRecord, ResultOrObject, Zone, ZoneId};
use figment::{providers::Serialized, Figment};
use mockall_double::double;

mod client;

use super::DnsService;
use crate::config::ConfigurationError;

#[double]
use client::CloudflareClient;

pub struct CloudflareDnsService {
    client: CloudflareClient,
    ttl: u32,
    proxied: bool,
}

impl CloudflareDnsService {
    pub fn configure(figment: &Figment) -> Result<Option<Self>, ConfigurationError> {
        if figment.extract_inner::<bool>("cloudflare.enabled")? == false {
            return Ok(None);
        }
        let client = CloudflareClient::new(figment.extract_inner("cloudflare.api_key")?);
        let ttl = figment.extract_inner("cloudflare.dns_ttl")?;
        let proxied = figment.extract_inner("cloudflare.proxied")?;
        Ok(Some(Self {
            client,
            ttl,
            proxied,
        }))
    }

    pub fn figment_default_values() -> Figment {
        Figment::from(Serialized::default(
            "cloudflare",
            serde_json::json!({
                "dns_ttl": 1,
                "proxied": false,
                "enabled": false
            }),
        ))
    }
}

fn base_domain<'a>(domain_name: &'a str) -> &'a str {
    let num_parts = domain_name.split(".").count();
    if num_parts <= 2 {
        return domain_name;
    }
    let len: usize = domain_name
        .split(".")
        .take(num_parts - 2)
        .map(|l| l.len())
        .sum::<usize>()
        + num_parts
        - 2;
    &domain_name[len..]
}

#[test]
fn test_base_domain() {
    assert_eq!(base_domain("example.com"), "example.com");
    assert_eq!(base_domain("hello.example.com"), "example.com");
    assert_eq!(base_domain("com"), "com");
}

impl CloudflareDnsService {
    fn get_zone(&self, domain_name: &str) -> anyhow::Result<Zone> {
        let zones = self.client.list_zones(base_domain(domain_name))?;
        if !zones.success {
            return Err(anyhow!(
                "Failed to list zones with following response {:?}",
                zones
            ));
        }

        if !matches!(zones.result, Some(ResultOrObject::Result(_))) {
            return Err(anyhow!(
                "List zones returned returned success but invalid result {:?}",
                zones
            ));
        }

        let mut zones = zones.result.unwrap().unwrap();
        if zones.is_empty() {
            return Err(anyhow!(
                "No zones listed for name {} - {:?}",
                domain_name,
                zones
            ));
        }

        if zones.len() > 1 {
            return Err(anyhow!("Too many zones for domain {:?}", zones));
        }

        Ok(zones.pop().unwrap())
    }

    fn get_existing_records(
        &self,
        zone_id: &ZoneId,
        domain_name: &str,
    ) -> anyhow::Result<Vec<GetDnsRecord>> {
        let records = self.client.list_dns_records(zone_id, domain_name, 1)?;
        if !records.success {
            return Err(anyhow!(
                "Could not fetch dns records with following body {:?}",
                records
            ));
        }

        if !matches!(records.result, Some(ResultOrObject::Result(_))) {
            return Err(anyhow!(
                "List records api call succeeded but gave invalid response {:?}",
                records
            ));
        }
        let records = records.result.unwrap().unwrap();

        Ok(records)
    }

    fn create_dns_record(
        &self,
        zone_id: &ZoneId,
        domain_name: &str,
        ip_address: std::net::IpAddr,
    ) -> anyhow::Result<()> {
        let request = CloudflareDnsRecordBody {
            type_: type_string(ip_address).to_owned(),
            name: domain_name.to_owned(),
            comment: Some("Record created by pond".to_string()),
            content: ip_address.to_string(),
            ttl: self.ttl,
            proxied: self.proxied,
        };

        let response = self.client.add_dns_record(zone_id, &request)?;
        if !response.success {
            Err(anyhow!("Failed to create dns record for zone {}, domain {} and ip {:?} with the following response {:?}", zone_id.0, domain_name, ip_address, response))
        } else {
            Ok(())
        }
    }

    fn update_dns_record(
        &self,
        zone_id: &ZoneId,
        record: GetDnsRecord,
        ip_address: std::net::IpAddr,
    ) -> anyhow::Result<()> {
        let request = CloudflareDnsRecordBody {
            type_: record.type_,
            name: record.name,
            comment: Some("Record updated by pond".to_string()),
            content: ip_address.to_string(),
            ttl: self.ttl,
            proxied: self.proxied,
        };

        let response = self
            .client
            .update_dns_record(zone_id, &record.id, &request)?;
        if !response.success {
            Err(anyhow!("Failed to create dns record for zone {}, domain {} and ip {:?} with the following response {:?}", zone_id.0, &request.name, ip_address, response))
        } else {
            Ok(())
        }
    }
}

fn type_string(ip_address: std::net::IpAddr) -> &'static str {
    if ip_address.is_ipv4() {
        "A"
    } else {
        "AAAA"
    }
}

impl DnsService for CloudflareDnsService {
    fn set_dns_record(
        &self,
        domain_name: &str,
        ip_address: std::net::IpAddr,
    ) -> anyhow::Result<()> {
        let zone = self.get_zone(domain_name)?;
        let records = self.get_existing_records(&zone.id, domain_name)?;
        let type_string = type_string(ip_address);
        let mut relevant_records: Vec<GetDnsRecord> = records
            .into_iter()
            .filter(|r| r.type_ == type_string)
            .collect();

        if relevant_records.is_empty() {
            self.create_dns_record(&zone.id, domain_name, ip_address)?;
        } else if relevant_records.len() == 1 {
            self.update_dns_record(&zone.id, relevant_records.pop().unwrap(), ip_address)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::{net::IpAddr, str::FromStr};

    use client::{CloudflareListRecordsResponse, CloudflareListZonesResponse, RecordId};

    use super::*;

    fn list_zones_response() -> CloudflareListZonesResponse {
        serde_json::from_str(client::testhelpers::LIST_ZONES_RESPONSE).unwrap()
    }

    #[test]
    fn the_happy_path_with_no_existing_records_works() {
        let mut mock = CloudflareClient::default();

        mock.expect_list_zones()
            .returning(|_| Ok(list_zones_response()));

        mock.expect_list_dns_records().returning(|_, _, _| {
            let mut records: CloudflareListRecordsResponse =
                serde_json::from_str(client::testhelpers::LIST_RECORDS_RESPONSE).unwrap();
            records.result = Some(ResultOrObject::Result(vec![]));
            Ok(records)
        });

        mock.expect_add_dns_record().times(1).returning(|_, _| {
            Ok(serde_json::from_str(client::testhelpers::ADD_RECORD_RESPONSE).unwrap())
        });

        let service = CloudflareDnsService {
            client: mock,
            ttl: 1,
            proxied: false,
        };

        service
            .set_dns_record("example.com", IpAddr::from_str("127.0.0.1").unwrap())
            .unwrap();
    }

    #[test]
    fn test_base_domain_with_sub_sub_domain() {
        assert_eq!(base_domain("sub.sub.example.com"), "example.com");
    }

    #[test]
    fn test_get_zone_with_multiple_zones() {
        let mut mock = CloudflareClient::default();

        mock.expect_list_zones().returning(|_| {
            let mut response = list_zones_response();

            response.result = Some(ResultOrObject::Result(vec![
                Zone {
                    id: ZoneId("1".to_string()),
                    name: "example.com".to_string(),
                    ..Default::default()
                },
                Zone {
                    id: ZoneId("2".to_string()),
                    name: "example.org".to_string(),
                    ..Default::default()
                },
            ]));

            Ok(response)
        });

        let service = CloudflareDnsService {
            client: mock,
            ttl: 1,
            proxied: false,
        };

        let result = service.get_zone("example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_zone_with_no_zones() {
        let mut mock = CloudflareClient::default();

        mock.expect_list_zones().returning(|_| {
            let mut response = list_zones_response();
            response.result = Some(ResultOrObject::Result(vec![]));
            Ok(response)
        });

        let service = CloudflareDnsService {
            client: mock,
            ttl: 1,
            proxied: false,
        };

        let result = service.get_zone("example.com");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_existing_records_with_no_records() {
        let mut mock = CloudflareClient::default();

        mock.expect_list_dns_records().returning(|_, _, _| {
            let mut response: CloudflareListRecordsResponse =
                serde_json::from_str(client::testhelpers::LIST_RECORDS_RESPONSE).unwrap();
            response.result = Some(ResultOrObject::Result(vec![]));
            Ok(response)
        });

        let service = CloudflareDnsService {
            client: mock,
            ttl: 1,
            proxied: false,
        };

        let result = service.get_existing_records(&ZoneId("zone_id".to_string()), "example.com");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_create_dns_record_failure() {
        let mut mock = CloudflareClient::default();

        mock.expect_add_dns_record().returning(|_, _| {
            Ok(serde_json::from_str(client::testhelpers::ADD_RECORD_FAILURE_RESPONSE).unwrap())
        });

        let service = CloudflareDnsService {
            client: mock,
            ttl: 1,
            proxied: false,
        };

        let result = service.create_dns_record(
            &ZoneId("zone_id".to_string()),
            "example.com",
            IpAddr::from_str("127.0.0.1").unwrap(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_update_dns_record_failure() {
        let mut mock = CloudflareClient::default();

        mock.expect_update_dns_record().returning(|_, _, _| {
            Ok(serde_json::from_str(client::testhelpers::ADD_RECORD_FAILURE_RESPONSE).unwrap())
        });

        let service = CloudflareDnsService {
            client: mock,
            ttl: 1,
            proxied: false,
        };

        let record = GetDnsRecord {
            id: RecordId("record_id".to_string()),
            type_: "A".to_string(),
            name: "example.com".to_string(),
            content: "127.0.0.1".to_string(),
            ttl: 1,
            proxied: true,
        };

        let result = service.update_dns_record(
            &ZoneId("zone_id".to_string()),
            record,
            IpAddr::from_str("127.0.0.1").unwrap(),
        );
        assert!(result.is_err());
    }
}
