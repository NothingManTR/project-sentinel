use argus_core::types::ShodanHostInfo;
use serde::Deserialize;
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::task::JoinSet;

const CRITICAL_PORTS: &[u16] = &[
    21, 22, 23, 25, 53, 80, 110, 143, 443, 445, 3306, 3389, 5432, 6379, 8080, 8443,
];

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct InternetDbResponse {
    #[serde(default)]
    ip: String,
    #[serde(default)]
    hostnames: Vec<String>,
    #[serde(default)]
    ports: Vec<u16>,
    #[serde(default)]
    cpes: Vec<String>,
    #[serde(default)]
    vulns: Vec<String>,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize, Debug)]
struct IpApiResponse {
    status: Option<String>,
    country: Option<String>,
    city: Option<String>,
    isp: Option<String>,
    #[serde(rename = "as")]
    as_num: Option<String>,
}

#[derive(Default)]
struct RipeInfo {
    prefix: Option<String>,
    asns: Vec<String>,
    holder: Option<String>,
    country: Option<String>,
}

pub struct ShodanClient {
    api_key: Option<String>,
    client: reqwest::Client,
}

impl ShodanClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            client: reqwest::Client::builder()
                .local_address(IpAddr::V4(Ipv4Addr::UNSPECIFIED))
                .timeout(Duration::from_secs(6))
                .user_agent("Project-Sentinel/0.1")
                .build()
                .unwrap_or_default(),
        }
    }

    /// Belirtilen IP adresi veya alan adı için Shodan + RIPE Stat BGP + Canlı Asenkron 16-Port & Banner Kesfi yapar
    pub async fn lookup_host(&self, target: &str) -> anyhow::Result<ShodanHostInfo> {
        let target = target.trim();
        let is_domain = target.parse::<IpAddr>().is_err();

        let ip = if is_domain {
            if let Ok(addrs) = tokio::net::lookup_host((target, 443)).await {
                let addr_list: Vec<SocketAddr> = addrs.collect();
                addr_list
                    .iter()
                    .find(|a| a.is_ipv4())
                    .or_else(|| addr_list.first())
                    .map(|a| a.ip().to_string())
                    .unwrap_or_else(|| target.to_string())
            } else if let Some(resolved) = self.resolve_domain(target).await {
                resolved
            } else {
                target.to_string()
            }
        } else {
            target.to_string()
        };

        let mut info = ShodanHostInfo {
            ip: ip.clone(),
            ..Default::default()
        };

        // Katman 1..4 ayni anda paralel calisir (toplam ~1.5 - 3 sn)
        let (idb_res, ripe_res, tcp_res, geo_res) = tokio::join!(
            self.fetch_internetdb(&ip),
            self.fetch_ripe_stat(&ip),
            Self::scan_live_ports(&ip, target),
            self.fetch_geoip(&ip),
        );

        // Katman 1: Shodan InternetDB (erisim varsa)
        if let Some(data) = idb_res {
            info.hostnames = data.hostnames;
            info.ports = data.ports;
            info.cpes = data.cpes;
            info.vulns = data.vulns;
            info.tags = data.tags;
        }

        if is_domain && !target.is_empty() && !info.hostnames.iter().any(|h| h == target) {
            info.hostnames.push(target.to_string());
        }

        // Katman 2: RIPE Stat BGP Prefix & ASN (anahtarsiz / engelsiz)
        if let Some(ripe) = ripe_res {
            if let Some(pfx) = ripe.prefix {
                let tag = format!("BGP:{}", pfx);
                if !info.tags.contains(&tag) {
                    info.tags.push(tag);
                }
            }
            for asn in &ripe.asns {
                let tag = format!("AS{}", asn);
                if !info.tags.contains(&tag) {
                    info.tags.push(tag);
                }
            }
            if info.isp.is_none() {
                let asn_label = ripe.asns.first().map(|a| format!("AS{}", a));
                info.isp = match (ripe.holder, asn_label) {
                    (Some(h), Some(a)) => Some(format!("{} ({})", h, a)),
                    (Some(h), None) => Some(h),
                    (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
            }
            if info.country.is_none() {
                info.country = ripe.country;
            }
        }

        // Katman 3: Canli 16-Port TCP & Servis Banner Yoklama
        let (live_ports, live_banners) = tcp_res;
        if !live_ports.is_empty() {
            let mut merged_ports: BTreeSet<u16> = info.ports.iter().copied().collect();
            for p in live_ports {
                merged_ports.insert(p);
            }
            info.ports = merged_ports.into_iter().collect();
            if !info.tags.iter().any(|t| t == "CANLI-TCP-TARAMA") {
                info.tags.push("CANLI-TCP-TARAMA".to_string());
            }
        }
        for banner in live_banners {
            if !info.cpes.contains(&banner) {
                info.cpes.push(banner);
            }
        }

        // Katman 4: GeoIP (ipwho.is + yedek ip-api.com)
        if let Some((country, city, isp)) = geo_res {
            if country.is_some() {
                info.country = country;
            }
            if city.is_some() {
                info.city = city;
            }
            if isp.is_some() {
                info.isp = isp;
            }
        }

        Ok(info)
    }

    async fn fetch_internetdb(&self, ip: &str) -> Option<InternetDbResponse> {
        let db_url = format!("https://internetdb.shodan.io/{}", ip);
        let fut = async {
            let resp = self.client.get(&db_url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            resp.json::<InternetDbResponse>().await.ok()
        };
        tokio::time::timeout(Duration::from_secs(3), fut)
            .await
            .ok()
            .flatten()
    }

    async fn fetch_ripe_stat(&self, ip: &str) -> Option<RipeInfo> {
        let net_url = format!(
            "https://stat.ripe.net/data/network-info/data.json?resource={}",
            ip
        );
        let whois_url = format!("https://stat.ripe.net/data/whois/data.json?resource={}", ip);

        let net_fut = async {
            let resp = self.client.get(&net_url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            resp.json::<serde_json::Value>().await.ok()
        };
        let whois_fut = async {
            let resp = self.client.get(&whois_url).send().await.ok()?;
            if !resp.status().is_success() {
                return None;
            }
            resp.json::<serde_json::Value>().await.ok()
        };

        let (net_res, whois_res) = tokio::time::timeout(
            Duration::from_secs(5),
            async { tokio::join!(net_fut, whois_fut) },
        )
        .await
        .ok()?;

        let mut out = RipeInfo::default();
        if let Some(nj) = net_res {
            out.prefix = nj["data"]["prefix"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            if let Some(arr) = nj["data"]["asns"].as_array() {
                for a in arr.iter().take(3) {
                    if let Some(s) = a.as_str() {
                        out.asns.push(s.to_string());
                    } else if let Some(n) = a.as_u64() {
                        out.asns.push(n.to_string());
                    }
                }
            }
        }
        if let Some(wj) = whois_res {
            if let Some(records) = wj["data"]["records"].as_array() {
                for rec in records {
                    if let Some(fields) = rec.as_array() {
                        for f in fields {
                            let key = f["key"].as_str().unwrap_or("").to_ascii_lowercase();
                            let val = f["value"].as_str().unwrap_or("").trim();
                            if val.is_empty() {
                                continue;
                            }
                            if out.holder.is_none()
                                && matches!(key.as_str(), "org-name" | "owner" | "descr" | "netname" | "as-name")
                            {
                                out.holder = Some(val.to_string());
                            }
                            if out.country.is_none() && key == "country" {
                                out.country = Some(val.to_string());
                            }
                        }
                    }
                }
            }
        }

        if out.prefix.is_some() || !out.asns.is_empty() || out.holder.is_some() {
            Some(out)
        } else {
            None
        }
    }

    async fn scan_live_ports(ip_str: &str, host_hdr: &str) -> (Vec<u16>, Vec<String>) {
        let Ok(ip_addr) = ip_str.parse::<IpAddr>() else {
            return (Vec::new(), Vec::new());
        };
        let host_owned = host_hdr.to_string();
        let mut set = JoinSet::new();

        for &port in CRITICAL_PORTS {
            let h = host_owned.clone();
            set.spawn(async move {
                let sa = SocketAddr::new(ip_addr, port);
                let Ok(Ok(mut stream)) =
                    tokio::time::timeout(Duration::from_millis(850), TcpStream::connect(sa)).await
                else {
                    return None;
                };

                let mut banner = match port {
                    53 => Some("53/tcp: DNS".to_string()),
                    443 => Some("443/tcp: HTTPS/TLS".to_string()),
                    445 => Some("445/tcp: SMB".to_string()),
                    3306 => Some("3306/tcp: MySQL".to_string()),
                    3389 => Some("3389/tcp: RDP".to_string()),
                    5432 => Some("5432/tcp: PostgreSQL".to_string()),
                    6379 => Some("6379/tcp: Redis".to_string()),
                    8443 => Some("8443/tcp: HTTPS-Alt".to_string()),
                    80 | 8080 => {
                        let req = format!(
                            "HEAD / HTTP/1.0\r\nHost: {}\r\nConnection: close\r\n\r\n",
                            h
                        );
                        let _ = tokio::time::timeout(
                            Duration::from_millis(250),
                            stream.write_all(req.as_bytes()),
                        )
                        .await;
                        let mut buf = [0u8; 512];
                        let n = tokio::time::timeout(
                            Duration::from_millis(450),
                            stream.read(&mut buf),
                        )
                        .await
                        .ok()
                        .and_then(|r| r.ok())
                        .unwrap_or(0);
                        let txt = String::from_utf8_lossy(&buf[..n]);
                        let srv = txt
                            .lines()
                            .find(|l| l.to_ascii_lowercase().starts_with("server:"))
                            .map(|l| l[7..].trim().to_string());
                        match srv {
                            Some(s) if !s.is_empty() => Some(format!("{}/tcp: HTTP ({})", port, s)),
                            _ => Some(format!("{}/tcp: HTTP", port)),
                        }
                    }
                    21 | 22 | 23 | 25 | 110 | 143 => {
                        let mut buf = [0u8; 192];
                        let n = tokio::time::timeout(
                            Duration::from_millis(450),
                            stream.read(&mut buf),
                        )
                        .await
                        .ok()
                        .and_then(|r| r.ok())
                        .unwrap_or(0);
                        let line = String::from_utf8_lossy(&buf[..n])
                            .lines()
                            .next()
                            .unwrap_or("")
                            .trim()
                            .chars()
                            .take(48)
                            .collect::<String>();
                        if !line.is_empty() {
                            Some(format!("{}/tcp: {}", port, line))
                        } else {
                            Some(format!("{}/tcp: Acik", port))
                        }
                    }
                    _ => Some(format!("{}/tcp: Acik", port)),
                };
                if let Some(b) = banner.take() {
                    return Some((port, b));
                }
                Some((port, format!("{}/tcp: Acik", port)))
            });
        }

        let mut open_ports = Vec::new();
        let mut banners = Vec::new();
        let _ = tokio::time::timeout(Duration::from_millis(1400), async {
            while let Some(res) = set.join_next().await {
                if let Ok(Some((port, b))) = res {
                    open_ports.push(port);
                    banners.push(b);
                }
            }
        })
        .await;

        open_ports.sort_unstable();
        banners.sort();
        (open_ports, banners)
    }

    async fn fetch_geoip(&self, ip: &str) -> Option<(Option<String>, Option<String>, Option<String>)> {
        let ipwho_url = format!("https://ipwho.is/{}", ip);
        if let Ok(resp) = self.client.get(&ipwho_url).send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if json["success"].as_bool() != Some(false) {
                        let country = json["country"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(str::to_string);
                        let city = json["city"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .map(str::to_string);
                        let isp = json["connection"]["isp"]
                            .as_str()
                            .filter(|s| !s.is_empty())
                            .or_else(|| json["connection"]["org"].as_str().filter(|s| !s.is_empty()))
                            .map(str::to_string);
                        let asn = json["connection"]["asn"]
                            .as_u64()
                            .map(|n| format!("AS{}", n))
                            .or_else(|| {
                                json["connection"]["asn"]
                                    .as_str()
                                    .filter(|s| !s.is_empty())
                                    .map(str::to_string)
                            });

                        if country.is_some() || isp.is_some() {
                            let full_isp = match (isp, asn) {
                                (Some(i), Some(a)) => Some(format!("{} ({})", i, a)),
                                (Some(i), None) => Some(i),
                                (None, Some(a)) => Some(a),
                                (None, None) => None,
                            };
                            return Some((country, city, full_isp));
                        }
                    }
                }
            }
        }

        let geo_url = format!(
            "http://ip-api.com/json/{}?fields=status,country,city,isp,as",
            ip
        );
        if let Ok(resp) = self.client.get(&geo_url).send().await {
            if resp.status().is_success() {
                if let Ok(geo) = resp.json::<IpApiResponse>().await {
                    if geo.status.as_deref() == Some("success") {
                        return Some((geo.country, geo.city, geo.isp.or(geo.as_num)));
                    }
                }
            }
        }
        None
    }

    /// Shodan DNS Resolve
    pub async fn resolve_domain(&self, domain: &str) -> Option<String> {
        let key = self.api_key.as_deref()?;
        let url = format!(
            "https://api.shodan.io/dns/resolve?hostnames={}&key={}",
            domain, key
        );
        if let Ok(resp) = self.client.get(&url).send().await {
            if resp.status().is_success() {
                if let Ok(json) = resp.json::<serde_json::Value>().await {
                    if let Some(ip) = json.get(domain).and_then(|v| v.as_str()) {
                        return Some(ip.to_string());
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires live network access"]
    async fn live_hybrid_recon_bypasses_shodan_block() {
        let client = ShodanClient::new(None);
        let info = client.lookup_host("1.1.1.1").await.expect("lookup failed");
        println!("Hybrid recon result for 1.1.1.1: {:?}", info);
        assert!(
            !info.ports.is_empty(),
            "Expected live TCP probe to detect open ports (e.g. 53/80/443), got none"
        );
        assert!(
            info.isp.is_some() || !info.tags.is_empty(),
            "Expected BGP/ASN or ISP metadata from RIPE Stat / GeoIP"
        );
    }
}
