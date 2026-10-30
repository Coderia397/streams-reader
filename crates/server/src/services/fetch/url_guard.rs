//! Which URLs the fetch proxy may request. Workers cannot resolve DNS before connecting and Cloudflare already
//! refuses connections into private networks, so this guard works on the URL alone: public http(s) on the default
//! ports, no credentials, no internal host names and no IP literals outside public unicast space.

use std::net::{Ipv4Addr, Ipv6Addr};

use nolabs_core::proto::ApiError;
use url::{Host, Url};

const BLOCKED_SUFFIXES: &[&str] = &[".localhost", ".local", ".internal", ".lan", ".home.arpa", ".intranet"];

/// Parse and check a URL, returning it without its fragment.
pub fn validate_url(raw: &str) -> Result<Url, ApiError> {
    let refuse = |why: &str| Err(ApiError::bad_request(format!("cannot fetch this address: {why}")));
    let Ok(mut url) = Url::parse(raw.trim()) else {
        return refuse("not a valid URL");
    };
    if !matches!(url.scheme(), "http" | "https") {
        return refuse("only http and https are allowed");
    }
    if !url.username().is_empty() || url.password().is_some() {
        return refuse("credentials in URLs are not allowed");
    }
    if !matches!(url.port(), None | Some(80) | Some(443)) {
        return refuse("only ports 80 and 443 are allowed");
    }
    match url.host() {
        None => return refuse("no host"),
        Some(Host::Domain(domain)) => {
            let d = domain.trim_end_matches('.').to_ascii_lowercase();
            if d.is_empty() || !d.contains('.') {
                return refuse("not a public host name");
            }
            if d == "localhost" || BLOCKED_SUFFIXES.iter().any(|s| d.ends_with(s)) {
                return refuse("internal host name");
            }
        }
        Some(Host::Ipv4(ip)) if !is_public_v4(ip) => return refuse("private or reserved IP address"),
        Some(Host::Ipv6(ip)) if !is_public_v6(ip) => return refuse("private or reserved IP address"),
        Some(_) => {}
    }
    url.set_fragment(None);
    Ok(url)
}

fn in_v4(ip: Ipv4Addr, net: [u8; 4], prefix: u32) -> bool {
    let mask = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    (u32::from(ip) & mask) == (u32::from(Ipv4Addr::from(net)) & mask)
}

pub fn is_public_v4(ip: Ipv4Addr) -> bool {
    const BLOCKED: &[([u8; 4], u32)] = &[
        ([0, 0, 0, 0], 8),       // "this network"
        ([10, 0, 0, 0], 8),      // private
        ([100, 64, 0, 0], 10),   // CGNAT
        ([127, 0, 0, 0], 8),     // loopback
        ([169, 254, 0, 0], 16),  // link-local (cloud metadata)
        ([172, 16, 0, 0], 12),   // private
        ([192, 0, 0, 0], 24),    // IETF protocol assignments
        ([192, 0, 2, 0], 24),    // documentation
        ([192, 88, 99, 0], 24),  // 6to4 relay anycast
        ([192, 168, 0, 0], 16),  // private
        ([198, 18, 0, 0], 15),   // benchmarking
        ([198, 51, 100, 0], 24), // documentation
        ([203, 0, 113, 0], 24),  // documentation
        ([224, 0, 0, 0], 4),     // multicast
        ([240, 0, 0, 0], 4),     // reserved and broadcast
    ];
    !BLOCKED.iter().any(|(net, prefix)| in_v4(ip, *net, *prefix))
}

pub fn is_public_v6(ip: Ipv6Addr) -> bool {
    let s = ip.segments();
    let embedded_v4 = || Ipv4Addr::new((s[6] >> 8) as u8, s[6] as u8, (s[7] >> 8) as u8, s[7] as u8);
    if ip.is_unspecified() || ip.is_loopback() {
        return false;
    }
    match s[0] {
        // ::/96 (IPv4-compatible) and ::ffff:0:0/96 (IPv4-mapped): judge the embedded address.
        0 if s[1..5].iter().all(|&x| x == 0) && (s[5] == 0 || s[5] == 0xffff) => is_public_v4(embedded_v4()),
        0 => false,
        // 64:ff9b::/96 NAT64 and 64:ff9b:1::/48 local-use NAT64.
        0x64 if s[1] == 0xff9b => s[2..6].iter().all(|&x| x == 0) && is_public_v4(embedded_v4()),
        // 6to4: 2002:AABB:CCDD::/48 embeds AA.BB.CC.DD.
        0x2002 => is_public_v4(Ipv4Addr::new((s[1] >> 8) as u8, s[1] as u8, (s[2] >> 8) as u8, s[2] as u8)),
        // Teredo 2001::/32 and documentation 2001:db8::/32.
        0x2001 if s[1] == 0 || s[1] == 0xdb8 => false,
        // ULA fc00::/7, link-local fe80::/10, site-local fec0::/10, multicast ff00::/8.
        x if (x & 0xfe00) == 0xfc00 || (x & 0xffc0) == 0xfe80 || (x & 0xffc0) == 0xfec0 || (x & 0xff00) == 0xff00 => {
            false
        }
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_table() {
        let allowed = [
            "https://example.com/article?id=1#top",
            "http://example.com:80/",
            "https://example.com:443/x",
            "https://sub.example.co.uk./path",
            "https://93.184.216.34/",
            "https://[2606:4700::1111]/",
            "https://[::ffff:93.184.216.34]/",
            "https://xn--bcher-kva.example/",
        ];
        let refused = [
            "ftp://example.com/",
            "file:///etc/passwd",
            "javascript:alert(1)",
            "data:text/html,hi",
            "https://user:pw@example.com/",
            "https://user@example.com/",
            "https://example.com:8080/",
            "https://example.com:22/",
            "http://localhost/",
            "http://LOCALHOST./",
            "http://foo.localhost/",
            "http://printer.local/",
            "http://metadata.google.internal/",
            "http://router.lan/",
            "http://nas.home.arpa/",
            "http://intranet/",
            "http://127.0.0.1/",
            "http://127.1/",
            "http://2130706433/",
            "http://0x7f000001/",
            "http://0.0.0.0/",
            "http://10.1.2.3/",
            "http://172.16.0.1/",
            "http://172.31.255.255/",
            "http://192.168.1.1/",
            "http://169.254.169.254/latest/meta-data/",
            "http://100.64.0.1/",
            "http://100.127.255.255/",
            "http://224.0.0.1/",
            "http://255.255.255.255/",
            "http://[::1]/",
            "http://[::]/",
            "http://[fc00::1]/",
            "http://[fd12:3456::1]/",
            "http://[fe80::1]/",
            "http://[ff02::1]/",
            "http://[::ffff:127.0.0.1]/",
            "http://[::ffff:10.0.0.1]/",
            "http://[64:ff9b::a9fe:a9fe]/",
            "http://[2002:c0a8:0101::1]/",
            "http://[2001:db8::1]/",
            "not a url",
            "",
        ];
        for u in allowed {
            assert!(validate_url(u).is_ok(), "should allow {u}");
        }
        for u in refused {
            assert!(validate_url(u).is_err(), "should refuse {u}");
        }
        assert_eq!(validate_url("https://example.com/a#frag").unwrap().as_str(), "https://example.com/a");
    }

    #[test]
    fn edges_of_private_ranges_are_public() {
        assert!(is_public_v4(Ipv4Addr::new(172, 15, 255, 255)));
        assert!(is_public_v4(Ipv4Addr::new(172, 32, 0, 0)));
        assert!(is_public_v4(Ipv4Addr::new(100, 63, 255, 255)));
        assert!(is_public_v4(Ipv4Addr::new(100, 128, 0, 0)));
        assert!(is_public_v4(Ipv4Addr::new(11, 0, 0, 0)));
        assert!(is_public_v6("2606:4700:4700::1111".parse().unwrap()));
        assert!(is_public_v6("64:ff9b::808:808".parse().unwrap()));
        assert!(is_public_v6("2002:0808:0808::1".parse().unwrap()));
    }
}
