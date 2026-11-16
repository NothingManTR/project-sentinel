use argus_core::types::ShodanHostInfo;
use serde::Deserialize;

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

pub struct ShodanClient {
    api_key: Option<String>,
    client: reqwest::Client,
}

impl ShodanClient {
    pub fn new(api_key: Option<String>) -> Self {
        Self {
            api_key,
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Belirtilen IP adresi veya alan adı için açık portlar, zafiyetler ve servis bilgilerini getirir
    pub async fn lookup_host(&self, target: &str) -> anyhow::Result<ShodanHostInfo> {
        let target = target.trim();
        let is_domain = target.parse::<std::net::IpAddr>().is_err();

        let ip = if is_domain {
            if let Some(resolved) = self.resolve_domain(target).await {
                resolved
            } else if let Ok(addrs) = tokio::net::lookup_host((target, 80)).await {
                let addr_list: Vec<std::net::SocketAddr> = addrs.collect();
                addr_list
                    .iter()
                    .find(|a| a.is_ipv4())
                    .or_else(|| addr_list.first())
                    .map(|a| a.ip().to_string())
                    .unwrap_or_else(|| target.to_string())
            } else {
                target.to_string()
            }
        } else {
            target.to_string()
        };

        // 1. Shodan InternetDB (Hızlı, açık portlar ve CVE'ler)
        let db_url = format!("https://internetdb.shodan.io/{}", ip);
        let mut info = ShodanHostInfo {
            ip: ip.clone(),
            ..Default::default()
        };

        if let Ok(resp) = self.client.get(&db_url).send().await {
            if resp.status().is_success() {
                if let Ok(data) = resp.json::<InternetDbResponse>().await {
                    info.hostnames = data.hostnames;
                    info.ports = data.ports;
                    info.cpes = data.cpes;
                    info.vulns = data.vulns;
                    info.tags = data.tags;
                }
            }
        }

        if is_domain && !target.is_empty() && !info.hostnames.iter().any(|h| h == target) {
            info.hostnames.push(target.to_string());
        }

        // 2. Birincil: HTTPS üzerinden ipwho.is ile ASN, ISP ve Konum Tamamlama
        let mut geo_resolved = false;
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
                            info.country = country;
                            info.city = city;
                            info.isp = match (isp, asn) {
                                (Some(i), Some(a)) => Some(format!("{} ({})", i, a)),
                                (Some(i), None) => Some(i),
                                (None, Some(a)) => Some(a),
                                (None, None) => None,
                            };
                            geo_resolved = true;
                        }
                    }
                }
            }
        }

        // 3. Yedek: IP-API ile Konum Tamamlama
        if !geo_resolved {
            let geo_url = format!("http://ip-api.com/json/{}?fields=status,country,city,isp,as", ip);
            if let Ok(resp) = self.client.get(&geo_url).send().await {
                if resp.status().is_success() {
                    if let Ok(geo) = resp.json::<IpApiResponse>().await {
                        if geo.status.as_deref() == Some("success") {
                            info.country = geo.country;
                            info.city = geo.city;
                            info.isp = geo.isp.or(geo.as_num);
                        }
                    }
                }
            }
        }

        Ok(info)
    }

    /// Shodan DNS Resolve
    pub async fn resolve_domain(&self, domain: &str) -> Option<String> {
        let key = self.api_key.as_deref()?;
        let url = format!("https://api.shodan.io/dns/resolve?hostnames={}&key={}", domain, key);
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
