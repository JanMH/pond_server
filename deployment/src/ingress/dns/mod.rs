use std::{
    collections::HashSet,
    net::{IpAddr, ToSocketAddrs},
    time::Instant,
};

pub mod cloudflare;

#[cfg(test)]
use mockall::{automock, predicate::*};

#[cfg_attr(test, automock)]
pub trait DnsService {
    fn set_dns_record(&self, domain_name: &str, ip_address: IpAddr) -> anyhow::Result<()>;
}

impl DnsService for Box<dyn DnsService> {
    fn set_dns_record(&self, domain_name: &str, ip_address: IpAddr) -> anyhow::Result<()> {
        self.as_ref().set_dns_record(domain_name, ip_address)
    }
}

pub struct NoOpDnsService;

impl DnsService for NoOpDnsService {
    fn set_dns_record(&self, _domain_name: &str, _ip_address: IpAddr) -> anyhow::Result<()> {
        Ok(())
    }
}

// This will not work due to operating system caching.
// We need to use a library like hickory-dns to resolve the domain name.
pub fn wait_for_dns_records(
    domain_name: &str,
    ip_addresses: impl Iterator<Item = IpAddr>,
    timeout: std::time::Duration,
) -> anyhow::Result<()> {
    let wanted_addresses: HashSet<IpAddr> = ip_addresses.collect();
    let start = Instant::now();

    while start.elapsed() < timeout {
        let actual_addresses: HashSet<IpAddr> = match (domain_name, 80).to_socket_addrs() {
            Ok(addresses) => addresses.map(|add| add.ip()).collect(),
            Err(_) => HashSet::new(),
        };

        if wanted_addresses.is_subset(&actual_addresses) {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
    Err(anyhow::anyhow!("Timeout waiting for DNS records"))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_wait_for_dns_records_not_fulfilled() {
        let domain_name = "example.com";
        let ip_addresses = vec![IpAddr::V4(std::net::Ipv4Addr::new(127, 0, 0, 1))];
        let timeout = std::time::Duration::from_millis(200);
        let result = wait_for_dns_records(domain_name, ip_addresses.iter().cloned(), timeout);
        assert!(result.is_err());
    }

    #[test]
    fn test_wait_for_dns_records_fulfilled() {
        let domain_name = "example.com";

        let ip_addresses = vec![(domain_name, 80)
            .to_socket_addrs()
            .unwrap()
            .next()
            .unwrap()
            .ip()];
        let timeout = std::time::Duration::from_secs(1);
        let result = wait_for_dns_records(domain_name, ip_addresses.iter().cloned(), timeout);
        assert!(result.is_ok());
    }
}
