use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde_json::Value;
use tracing::{info, warn};

use crate::http::Client;
use crate::station::{Station, StreamVariant};

const YP_URL: &str = "https://dir.xiph.org/yp.xml";
const FAVICON_CONCURRENCY: usize = 96;

/// Server names the Icecast status page / source clients default to; they carry
/// no information and are common in the YP feed.
const JUNK_NAMES: &[&str] = &[
    "my station name",
    "default stream",
    "orban opticodec-pc encoder",
    "z/ipstream r/1",
    "online radio",
    "unspecified name",
    "no name",
    "this is my server name",
    "stream name",
    "new stream",
    "unnamed",
    "icecast",
    "icecast2",
    "test",
    "demo",
    "unknown",
    "none",
    "null",
    "(null)",
    "a stream",
    "my stream",
    "stream",
    "mbstudio",
    "(mbstudio)",
];

/// Genre tokens that aren't useful as tags.
const JUNK_TAGS: &[&str] = &[
    "various",
    "other",
    "misc",
    "null",
    "unknown",
    "unspecified",
    "assorted",
    "none",
    "na",
];

/// Streaming-platform hosts: their favicon is the provider's, not the station's.
const PROVIDER_HOSTS: &[&str] = &[
    "pro-fhi.net",
    "yesstreaming.net",
    "streamerr.co",
    "newradio.it",
    "radiomedia.fr",
    "cdnstream.com",
    "streamingmedia.it",
    "streamguys",
    "laut.fm",
    "shoutcast",
    "zeno.fm",
    "radionomy",
    "live-streams.nl",
    "xcast.com.br",
    "radiotoolkit",
    "streamakaci",
    "infomaniak",
    "streamupsolutions",
    "listen2myradio",
    "streamon.fm",
    "radioking",
    "airtime.pro",
    "radiojar",
    "wix.com",
    "fbcdn.net",
    "facebook.com",
];

pub async fn discover(client: &Client) -> Vec<Station> {
    let xml = match client.get(YP_URL).send().await {
        Ok(response) => match response.text().await {
            Ok(text) => text,
            Err(error) => {
                warn!(provider = "icecast-yp", %error, "Failed to read Yellow Pages feed");
                return vec![];
            }
        },
        Err(error) => {
            warn!(provider = "icecast-yp", %error, "Failed to fetch Yellow Pages feed");
            return vec![];
        }
    };

    let mut stations = parse_feed(&xml);
    info!(
        provider = "icecast-yp",
        count = stations.len(),
        "Discovered stations"
    );

    enrich_favicons(&mut stations).await;
    let with_logo = stations.iter().filter(|s| s.logo_url.is_some()).count();
    info!(
        provider = "icecast-yp",
        with_logo, "Favicon enrichment complete"
    );

    enrich_countries(&mut stations).await;
    let with_country = stations.iter().filter(|s| s.country_code.is_some()).count();
    info!(
        provider = "icecast-yp",
        with_country, "Country enrichment complete"
    );

    stations
}

/// Parse the flat `<entry>` list, keeping entries with a usable `listen_url`.
///
/// An Icecast station is often published as several mounts — one per
/// bitrate/codec — with no canonical single URL. Group those into one entry
/// per station (host + name with the bitrate/codec suffix stripped), keeping
/// the highest-bitrate mount as `stream_url` and the full set as `streams`.
fn parse_feed(xml: &str) -> Vec<Station> {
    struct Group {
        name: String,
        tags: Vec<String>,
        primary_bitrate: u32,
        primary_url: String,
        variants: Vec<StreamVariant>,
    }

    let mut seen_urls = HashSet::new();
    let mut groups: HashMap<(String, String), Group> = HashMap::new();

    for entry in xml.split("<entry>").skip(1) {
        let Some(stream_url) = tag(entry, "listen_url") else {
            continue;
        };
        if !stream_url.starts_with("http") {
            continue;
        }
        let name = decode_entities(&tag(entry, "server_name").unwrap_or_default());
        if is_junk_name(&name) {
            continue;
        }
        if !seen_urls.insert(normalise_url(&stream_url)) {
            continue;
        }

        let bitrate = tag(entry, "bitrate")
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        let format = tag(entry, "server_type").map(|value| codec_name(&value));
        let tags = tag(entry, "genre")
            .map(|genre| parse_tags(&decode_entities(&genre)))
            .unwrap_or_default();
        let host = host_of(&stream_url).unwrap_or_default();

        let variant = StreamVariant {
            url: stream_url.clone(),
            bitrate: (bitrate > 0).then_some(bitrate),
            format,
        };

        let group = groups
            .entry((host, name_key(&name)))
            .or_insert_with(|| Group {
                name: name.clone(),
                tags: Vec::new(),
                primary_bitrate: 0,
                primary_url: stream_url.clone(),
                variants: Vec::new(),
            });
        for tag in tags {
            if !group.tags.contains(&tag) {
                group.tags.push(tag);
            }
        }
        if !group.variants.iter().any(|v| v.url == stream_url) {
            group.variants.push(variant);
        }
        if bitrate >= group.primary_bitrate {
            group.primary_bitrate = bitrate;
            group.primary_url = stream_url;
        }
    }

    let mut stations: Vec<Station> = groups
        .into_values()
        .map(|group| Station {
            name: group.name,
            stream_url: group.primary_url,
            streams: if group.variants.len() > 1 {
                group.variants
            } else {
                Vec::new()
            },
            logo_url: None,
            country: None,
            country_code: None,
            tags: group.tags,
            description: None,
            provider: "icecast-yp".into(),
            provider_id: None,
            trusted: false,
        })
        .collect();
    stations.sort_by(|a, b| a.name.cmp(&b.name));
    stations
}

/// Case- and punctuation-insensitive key for grouping a station's mounts,
/// with any bitrate/codec suffix removed so `Radio X 128kbit` and
/// `Radio X 64kbit` group together.
fn name_key(name: &str) -> String {
    name.to_ascii_lowercase()
        .split_whitespace()
        .filter(|word| {
            !word.contains("kbit")
                && !word.contains("kbps")
                && !matches!(*word, "mp3" | "aac" | "aacp" | "ogg" | "opus" | "hls")
        })
        .collect::<Vec<_>>()
        .join("")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// Map an Icecast `server_type` MIME to a short codec name.
fn codec_name(server_type: &str) -> String {
    match server_type.to_ascii_lowercase().as_str() {
        "audio/mpeg" => "mp3",
        "audio/aac" | "audio/aacp" | "audio/x-aac" => "aac",
        "audio/ogg" | "application/ogg" => "ogg",
        "audio/opus" => "opus",
        other => other,
    }
    .to_string()
}

fn tag(entry: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = entry.find(&open)? + open.len();
    let end = entry[start..].find(&close)? + start;
    let value = entry[start..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn is_junk_name(name: &str) -> bool {
    let name = name.trim().to_ascii_lowercase();
    if name.is_empty() || JUNK_NAMES.contains(&name.as_str()) {
        return true;
    }
    const JUNK_SUBSTRINGS: &[&str] = &[
        "mbstudio",
        "opticodec",
        "z/ipstream",
        "default stream",
        "my station name",
    ];
    JUNK_SUBSTRINGS.iter().any(|needle| name.contains(needle))
}

/// Decode the XML character/entity references the YP feed leaves in names and
/// genres (e.g. `&#39;`, `&amp;`, `&#x9;`).
fn decode_entities(input: &str) -> String {
    if !input.contains('&') {
        return input.to_string();
    }
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(index) = rest.find('&') {
        out.push_str(&rest[..index]);
        let after = &rest[index + 1..];
        let Some(semi) = after.find(';') else {
            out.push_str(&rest[index..]);
            return out;
        };
        let entity = &after[..semi];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .and_then(|hex| u32::from_str_radix(hex, 16).ok())
                .or_else(|| entity.strip_prefix('#').and_then(|dec| dec.parse().ok()))
                .and_then(char::from_u32),
        };
        match decoded {
            Some(ch) => {
                out.push(ch);
                rest = &after[semi + 1..];
            }
            None => {
                out.push_str(&rest[index..index + 1 + semi + 1]);
                rest = &after[semi + 1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn parse_tags(genre: &str) -> Vec<String> {
    let mut tags: Vec<String> = genre
        .split([',', ';', '/', '|'])
        .flat_map(|part| part.split_whitespace())
        .map(|word| word.trim().trim_matches('.').to_ascii_lowercase())
        .filter(|word| word.len() >= 2 && !JUNK_TAGS.contains(&word.as_str()))
        .collect();
    tags.sort();
    tags.dedup();
    tags
}

fn normalise_url(url: &str) -> String {
    url.to_ascii_lowercase()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('?')
        .next()
        .unwrap_or("")
        .trim_end_matches(['/', ';'])
        .to_string()
}

/// For each station, try to read the station's own website from its Icecast
/// `status-json` (`server_url`), then pull an icon from that site. Only the
/// station's own site is used — never the streaming host's favicon.
async fn enrich_favicons(stations: &mut [Station]) {
    let Ok(http) = reqwest::Client::builder()
        .timeout(Duration::from_secs(4))
        .user_agent("aerial-registry/0.1 (+icecast-yp)")
        .build()
    else {
        return;
    };

    let urls: Vec<String> = stations
        .iter()
        .map(|station| station.stream_url.clone())
        .collect();

    let results: Vec<(usize, Option<String>)> = stream::iter(urls.into_iter().enumerate())
        .map(|(index, stream_url)| {
            let http = http.clone();
            async move { (index, favicon_for(&http, &stream_url).await) }
        })
        .buffer_unordered(FAVICON_CONCURRENCY)
        .collect()
        .await;

    for (index, icon) in results {
        stations[index].logo_url = icon;
    }
}

/// Infer each station's country from its stream host's IP address. Icecast
/// stations carry no country of their own, and the stream host is usually the
/// station's own server, so the IP's geolocated country is a reasonable proxy.
async fn enrich_countries(stations: &mut [Station]) {
    let Ok(http) = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("aerial-registry/0.1 (+icecast-yp)")
        .build()
    else {
        return;
    };

    let mut by_host: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, station) in stations.iter().enumerate() {
        if let Some(host) = host_of(&station.stream_url) {
            by_host.entry(host).or_default().push(index);
        }
    }

    let hosts: Vec<String> = by_host.keys().cloned().collect();
    let resolved: Vec<(String, Option<IpAddr>)> = stream::iter(hosts)
        .map(|host| async move {
            let ip = tokio::net::lookup_host((host.as_str(), 0))
                .await
                .ok()
                .and_then(|mut addrs| addrs.next())
                .map(|addr| addr.ip());
            (host, ip)
        })
        .buffer_unordered(64)
        .collect()
        .await;

    let mut ip_hosts: HashMap<IpAddr, Vec<String>> = HashMap::new();
    for (host, ip) in resolved {
        if let Some(ip) = ip {
            ip_hosts.entry(ip).or_default().push(host);
        }
    }
    let ips: Vec<IpAddr> = ip_hosts.keys().copied().collect();

    // ip-api's free batch endpoint takes up to 100 IPs per request and is rate
    // limited, so space the batches out.
    let mut geo: HashMap<IpAddr, (String, String)> = HashMap::new();
    for chunk in ips.chunks(100) {
        let body: Vec<Value> = chunk
            .iter()
            .map(|ip| serde_json::json!({ "query": ip.to_string() }))
            .collect();
        let request = http
            .post("http://ip-api.com/batch?fields=query,country,countryCode")
            .json(&body);
        if let Ok(response) = request.send().await
            && let Ok(items) = response.json::<Vec<Value>>().await
        {
            for item in items {
                let (Some(query), Some(country), Some(code)) = (
                    item.get("query").and_then(Value::as_str),
                    item.get("country").and_then(Value::as_str),
                    item.get("countryCode").and_then(Value::as_str),
                ) else {
                    continue;
                };
                if let Ok(ip) = query.parse::<IpAddr>() {
                    geo.insert(ip, (country.to_string(), code.to_string()));
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(4)).await;
    }

    for (ip, hosts) in ip_hosts {
        let Some((country, code)) = geo.get(&ip) else {
            continue;
        };
        for host in hosts {
            let Some(indices) = by_host.get(&host) else {
                continue;
            };
            for &index in indices {
                stations[index].country = Some(country.clone());
                stations[index].country_code = Some(code.clone());
            }
        }
    }
}

fn host_of(url: &str) -> Option<String> {
    reqwest::Url::parse(url)
        .ok()?
        .host_str()
        .map(|host| host.to_ascii_lowercase())
}

async fn favicon_for(http: &reqwest::Client, stream_url: &str) -> Option<String> {
    let parsed = reqwest::Url::parse(stream_url).ok()?;
    let host = parsed.host_str()?.to_string();
    let port = parsed.port_or_known_default()?;

    let server_url = station_site(http, &host, port).await?;

    // A `server_url` that just points back at the stream server isn't the
    // station's website — its favicon would be the streaming host's.
    if site_host(&server_url).as_deref() == Some(host.as_str()) {
        return None;
    }

    let icon = fetch_site_icon(http, &server_url).await?;

    if is_provider_host(&icon) {
        return None;
    }
    Some(icon)
}

/// Host of a URL that may or may not carry a scheme.
fn site_host(site: &str) -> Option<String> {
    let with_scheme = if site.starts_with("http") {
        site.to_string()
    } else {
        format!("http://{site}")
    };
    reqwest::Url::parse(&with_scheme)
        .ok()?
        .host_str()
        .map(|h| h.to_ascii_lowercase())
}

/// Read `server_url` (the station's own website) from the Icecast JSON stats.
async fn station_site(http: &reqwest::Client, host: &str, port: u16) -> Option<String> {
    for scheme in ["https", "http"] {
        let url = format!("{scheme}://{host}:{port}/status-json.xsl");
        let Ok(response) = http.get(&url).send().await else {
            continue;
        };
        if !response.status().is_success() {
            continue;
        }
        let Ok(value) = response.json::<Value>().await else {
            continue;
        };
        let sources = value.pointer("/icestats/source")?;
        let first = sources
            .as_array()
            .and_then(|a| a.first())
            .or(Some(sources))?;
        if let Some(site) = first.get("server_url").and_then(Value::as_str)
            && !site.is_empty()
        {
            return Some(site.to_string());
        }
    }
    None
}

async fn fetch_site_icon(http: &reqwest::Client, site: &str) -> Option<String> {
    let base = if site.starts_with("http") {
        site.to_string()
    } else {
        format!("http://{site}")
    };
    let url = reqwest::Url::parse(&base).ok()?;
    if matches!(url.host_str(), Some("localhost") | Some("127.0.0.1")) {
        return None;
    }

    let response = http.get(url.clone()).send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let body = response.text().await.ok()?;
    let mut end = body.len().min(200_000);
    while end > 0 && !body.is_char_boundary(end) {
        end -= 1;
    }
    let html = &body[..end];

    for property in [
        "og:image",
        "og:image:url",
        "og:image:secure_url",
        "twitter:image",
        "twitter:image:src",
    ] {
        if let Some(image) = find_meta_content(html, property)
            && let Some(resolved) = resolve(&url, &image)
        {
            return Some(resolved);
        }
    }
    for rel in [
        "apple-touch-icon",
        "apple-touch-icon-precomposed",
        "icon",
        "shortcut icon",
        "image_src",
        "mask-icon",
    ] {
        if let Some(href) = find_link_href(html, rel)
            && let Some(resolved) = resolve(&url, &href)
        {
            return Some(resolved);
        }
    }
    None
}

fn resolve(base: &reqwest::Url, href: &str) -> Option<String> {
    base.join(href).ok().map(|u| u.to_string())
}

fn find_meta_content(html: &str, property: &str) -> Option<String> {
    for tag in html.split('<') {
        let lower = tag.to_ascii_lowercase();
        if !lower.starts_with("meta") {
            continue;
        }
        let tag = tag.split('>').next().unwrap_or("");
        let prop = attr(tag, "property").or_else(|| attr(tag, "name"));
        if prop.as_deref() == Some(property)
            && let Some(content) = attr(tag, "content")
        {
            return Some(content);
        }
    }
    None
}

fn find_link_href(html: &str, rel: &str) -> Option<String> {
    for tag in html.split('<') {
        let lower = tag.to_ascii_lowercase();
        if !lower.starts_with("link") {
            continue;
        }
        let tag = tag.split('>').next().unwrap_or("");
        if let Some(value) = attr(tag, "rel")
            && value
                .to_ascii_lowercase()
                .split_whitespace()
                .any(|word| word == rel)
        {
            return attr(tag, "href");
        }
    }
    None
}

/// Extract an attribute value from a single HTML tag, e.g. `content="…"`.
fn attr(tag: &str, name: &str) -> Option<String> {
    let key = format!("{name}=");
    let index = tag.to_ascii_lowercase().find(&key)? + key.len();
    let rest = &tag[index..];
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    let value = rest[..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

fn is_provider_host(url: &str) -> bool {
    let Ok(parsed) = reqwest::Url::parse(url) else {
        return true;
    };
    let host = parsed.host_str().unwrap_or("").to_ascii_lowercase();
    if host.is_empty() || host == "localhost" {
        return true;
    }
    PROVIDER_HOSTS.iter().any(|blocked| host.contains(blocked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_xml_entities() {
        assert_eq!(decode_entities("&#39;t Is Vloms"), "'t Is Vloms");
        assert_eq!(decode_entities("&amp;Rho;&amp;Alpha;"), "&Rho;&Alpha;");
        assert_eq!(decode_entities("Radio&#x9;X"), "Radio\tX");
        assert_eq!(decode_entities("plain name"), "plain name");
    }

    #[test]
    fn recognises_junk_names() {
        assert!(is_junk_name("My Station name"));
        assert!(is_junk_name("(null)"));
        assert!(is_junk_name("Orban Opticodec-PC Encoder"));
        assert!(is_junk_name("   "));
        assert!(!is_junk_name("Radio Marina"));
    }

    #[test]
    fn splits_and_filters_genre_tags() {
        assert_eq!(parse_tags("pop, rock; Various"), vec!["pop", "rock"]);
        assert!(parse_tags("various").is_empty());
        let drum = parse_tags("Drum and Bass");
        assert!(drum.contains(&"drum".to_string()) && drum.contains(&"bass".to_string()));
    }

    #[test]
    fn parses_feed_keeping_only_usable_entries() {
        let xml = "<directory>\
            <entry><server_name>Radio X</server_name><listen_url>http://a/b</listen_url><genre>pop, rock</genre></entry>\
            <entry><server_name>Default Stream</server_name><listen_url>http://c/d</listen_url><genre>various</genre></entry>\
            <entry><server_name>Radio Y</server_name><listen_url>http://a/b</listen_url><genre>jazz</genre></entry>\
            </directory>";
        let stations = parse_feed(xml);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].name, "Radio X");
        assert_eq!(stations[0].stream_url, "http://a/b");
        assert_eq!(
            stations[0].tags,
            vec!["pop".to_string(), "rock".to_string()]
        );
        assert_eq!(stations[0].provider, "icecast-yp");
        assert!(!stations[0].trusted);
    }

    #[test]
    fn collapses_same_station_mounts_to_the_highest_bitrate() {
        let xml = "<directory>\
            <entry><server_name>Radio X</server_name><listen_url>http://a/low</listen_url><bitrate>64</bitrate><genre>pop</genre></entry>\
            <entry><server_name>Radio X</server_name><listen_url>http://a/high</listen_url><bitrate>192</bitrate><genre>pop</genre></entry>\
            </directory>";
        let stations = parse_feed(xml);
        assert_eq!(stations.len(), 1);
        assert_eq!(stations[0].stream_url, "http://a/high");
    }

    #[test]
    fn rejects_provider_favicon_hosts() {
        assert!(is_provider_host("http://yesstreaming.net/favicon.ico"));
        assert!(is_provider_host("https://www.wix.com/favicon.ico"));
        assert!(!is_provider_host(
            "https://www.vinylsoundradio.com/logo.png"
        ));
    }
}
