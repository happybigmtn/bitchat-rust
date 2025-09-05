//! Geo and region extraction helpers for gateway routing

use std::net::IpAddr;

/// Derive a region hint from a JWT `region` claim if present
pub fn region_from_jwt_claim(region_claim: Option<&str>) -> Option<String> {
    region_claim.map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty())
}

/// Very rough IP-based region inference placeholder
/// Real implementation should use a GeoIP DB; here we stub by private ranges/local
pub fn region_from_ip(ip: IpAddr) -> Option<String> {
    match ip {
        IpAddr::V4(v4) => {
            let octets = v4.octets();
            // Local nets -> local region
            if octets[0] == 10 || (octets[0] == 192 && octets[1] == 168) || (octets[0] == 172 && (16..=31).contains(&octets[1])) {
                Some("local".to_string())
            } else {
                None
            }
        }
        IpAddr::V6(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_region_from_jwt_claim() {
        assert_eq!(region_from_jwt_claim(Some("IAD")).as_deref(), Some("iad"));
        assert!(region_from_jwt_claim(None).is_none());
    }

    #[test]
    fn test_region_from_ip_stub() {
        let ip: IpAddr = "192.168.1.5".parse().unwrap();
        assert_eq!(region_from_ip(ip).as_deref(), Some("local"));
    }
}

