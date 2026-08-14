//! Homepage hero photos: Pixieset gallery, else local files under `static/images/`.

use reqwest::header::{COOKIE, SET_COOKIE, USER_AGENT};
use serde::Deserialize;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

const CACHE_TTL: Duration = Duration::from_mins(30);
const MAX_PAGES: u32 = 10;
const DEFAULT_GALLERY_URL: &str =
    "https://shuttertheorymedia.pixieset.com/carolinasummerswing/";
/// Pixieset's gallery HTML is behind Cloudflare; this crawler UA is allowed through.
const GALLERY_USER_AGENT: &str = "WhatsApp/2.0";

/// Cached Pixieset (or local) homepage hero image URLs.
#[derive(Debug)]
pub struct HeroGallery {
    gallery_url: String,
    gallery_slug: String,
    client: reqwest::Client,
    cache: Mutex<HeroCache>,
}

#[derive(Debug, Default)]
struct HeroCache {
    urls: Vec<String>,
    fetched_at: Option<Instant>,
}

#[derive(Debug, Deserialize)]
struct LoadPhotosResponse {
    status: String,
    content: serde_json::Value,
    #[serde(rename = "isLastPage", default)]
    is_last_page: bool,
}

#[derive(Debug, Deserialize)]
struct PixiePhoto {
    #[serde(rename = "pathXlarge")]
    xlarge: Option<String>,
    #[serde(rename = "pathXxlarge")]
    xxlarge: Option<String>,
    #[serde(rename = "pathLarge")]
    large: Option<String>,
}

struct GalleryMeta {
    collection_id: String,
    collection_key: String,
    gallery_slug: String,
}

impl HeroGallery {
    /// Read `HERO_GALLERY_URL` / `HERO_GALLERY_SLUG` and build the HTTP client.
    pub fn init() -> Self {
        let gallery_url = std::env::var("HERO_GALLERY_URL").unwrap_or_else(|_| {
            DEFAULT_GALLERY_URL.to_string()
        });
        let gallery_slug = std::env::var("HERO_GALLERY_SLUG").unwrap_or_default();
        if gallery_url.is_empty() {
            info!("Pixieset hero gallery disabled (HERO_GALLERY_URL is empty).");
        } else {
            info!("Pixieset hero gallery: {gallery_url}");
        }
        let client = reqwest::ClientBuilder::new()
            .redirect(reqwest::redirect::Policy::limited(5))
            .timeout(Duration::from_secs(25))
            .build()
            .expect("hero gallery HTTP client should build");
        Self {
            gallery_url,
            gallery_slug,
            client,
            cache: Mutex::new(HeroCache::default()),
        }
    }

    /// Public gallery page the hero photos were loaded from. Empty if remote fetch is disabled.
    pub fn source_url(&self) -> &str {
        &self.gallery_url
    }

    /// Image URLs for the homepage rotator, cached for 30 minutes.
    pub async fn urls(&self) -> Vec<String> {
        {
            let cache = self.cache.lock().await;
            if let Some(fetched_at) = cache.fetched_at
                && fetched_at.elapsed() < CACHE_TTL
                && !cache.urls.is_empty()
            {
                return cache.urls.clone();
            }
        }

        let mut urls = Vec::new();
        if !self.gallery_url.is_empty() {
            match fetch_pixieset_gallery(&self.client, &self.gallery_url, &self.gallery_slug)
                .await
            {
                Ok(remote) if !remote.is_empty() => {
                    info!("Loaded {} Pixieset photos for the homepage hero", remote.len());
                    urls = remote;
                }
                Ok(_) => warn!("Pixieset gallery returned no photos; using local images"),
                Err(err) => warn!(%err, "Pixieset gallery fetch failed; using local images"),
            }
        }

        if urls.is_empty() {
            urls = local_hero_urls();
        }

        let mut cache = self.cache.lock().await;
        cache.urls.clone_from(&urls);
        cache.fetched_at = Some(Instant::now());
        urls
    }
}

fn local_hero_urls() -> Vec<String> {
    let dir = Path::new("static/images");
    let skip = [
        "dance_tech_logo",
        "default_pfp",
        "shopping_cart_icon",
    ];
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };

    let mut urls: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let ext = path
                .extension()
                .and_then(|e| e.to_str())
                .map(str::to_ascii_lowercase)?;
            if !matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "webp") {
                return None;
            }
            let name = path.file_name()?.to_str()?;
            if skip.iter().any(|prefix| name.starts_with(prefix)) {
                return None;
            }
            Some(format!("/static/images/{}", name.replace(' ', "%20")))
        })
        .collect();
    urls.sort();
    urls
}

async fn fetch_pixieset_gallery(
    client: &reqwest::Client,
    gallery_url: &str,
    slug_override: &str,
) -> Result<Vec<String>, String> {
    let page = client
        .get(gallery_url)
        .header(USER_AGENT, GALLERY_USER_AGENT)
        .send()
        .await
        .map_err(|err| err.to_string())?;
    if !page.status().is_success() {
        return Err(format!("gallery HTML {}", page.status()));
    }
    let cookies = cookies_from_headers(page.headers());
    let html = page.text().await.map_err(|err| err.to_string())?;
    if html.contains("Just a moment") {
        return Err("Cloudflare blocked the gallery page".to_string());
    }

    let meta = parse_gallery_meta(&html, slug_override)
        .ok_or_else(|| "could not parse PixiesetClient.init from gallery HTML".to_string())?;

    let origin = origin_of(gallery_url)?;
    let mut urls = Vec::new();

    for page_no in 1..=MAX_PAGES {
        let photos_url = format!("{origin}/client/loadphotos/");
        let res = client
            .get(&photos_url)
            .header(USER_AGENT, GALLERY_USER_AGENT)
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", gallery_url)
            .header(COOKIE, &cookies)
            .query(&[
                ("cuk", meta.collection_key.as_str()),
                ("cid", meta.collection_id.as_str()),
                ("gs", meta.gallery_slug.as_str()),
                ("fk", ""),
                ("page", &page_no.to_string()),
            ])
            .send()
            .await
            .map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("loadphotos {status}: {}", truncate(&body, 180)));
        }
        let parsed: LoadPhotosResponse = res.json().await.map_err(|err| err.to_string())?;
        if parsed.status != "success" {
            return Err(format!("loadphotos status {}", parsed.status));
        }
        for photo in photos_from_content(parsed.content)? {
            if let Some(src) = photo_src(&photo) {
                urls.push(src);
            }
        }
        if parsed.is_last_page {
            break;
        }
    }

    Ok(urls)
}

fn photos_from_content(content: serde_json::Value) -> Result<Vec<PixiePhoto>, String> {
    match content {
        serde_json::Value::String(raw) if raw.is_empty() => Ok(Vec::new()),
        serde_json::Value::String(raw) => {
            serde_json::from_str(&raw).map_err(|err| err.to_string())
        }
        serde_json::Value::Array(_) => {
            serde_json::from_value(content).map_err(|err| err.to_string())
        }
        other => Err(format!("unexpected loadphotos content: {other}")),
    }
}

fn photo_src(photo: &PixiePhoto) -> Option<String> {
    photo
        .xlarge
        .as_deref()
        .or(photo.xxlarge.as_deref())
        .or(photo.large.as_deref())
        .map(absolutize)
}

fn parse_gallery_meta(html: &str, slug_override: &str) -> Option<GalleryMeta> {
    let blob = html.find("PixiesetClient.init(").map(|i| &html[i..])?;
    let collection_id = js_int_field(blob, "collectionId")?;
    let collection_key = js_str_field(blob, "collectionUrlKey")?;
    let gallery_slug = if slug_override.is_empty() {
        js_str_field(blob, "currentGallery")?
    } else {
        slug_override.to_string()
    };
    Some(GalleryMeta {
        collection_id,
        collection_key,
        gallery_slug,
    })
}

fn js_str_field(blob: &str, key: &str) -> Option<String> {
    let needle = format!("'{key}':'");
    let rest = blob.split_once(&needle)?.1;
    let value = rest.split_once('\'')?.0;
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn js_int_field(blob: &str, key: &str) -> Option<String> {
    let needle = format!("'{key}':");
    let rest = blob.split_once(&needle)?.1;
    let value: String = rest.chars().take_while(char::is_ascii_digit).collect();
    if value.is_empty() { None } else { Some(value) }
}

fn cookies_from_headers(headers: &reqwest::header::HeaderMap) -> String {
    headers
        .get_all(SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .filter_map(|cookie| cookie.split(';').next())
        .collect::<Vec<_>>()
        .join("; ")
}

fn origin_of(url: &str) -> Result<String, String> {
    let parsed = url::Url::parse(url).map_err(|err| err.to_string())?;
    let origin = parsed.origin().ascii_serialization();
    if origin == "null" {
        Err("gallery URL has no origin".to_string())
    } else {
        Ok(origin)
    }
}

fn absolutize(src: &str) -> String {
    if src.starts_with("//") {
        format!("https:{src}")
    } else {
        src.to_string()
    }
}

fn truncate(s: &str, max: usize) -> &str {
    if s.len() <= max { s } else { &s[..max] }
}

#[cfg(test)]
mod tests {
    use super::{absolutize, js_int_field, js_str_field, parse_gallery_meta};

    const INIT: &str = "PixiesetClient.init({'page':'view_gallery','collectionId':119166914,'collectionUrlKey':'carolinasummerswing','currentGallery':'sneakpeaks'});";

    #[test]
    fn parse_pixieset_init_fields() {
        let meta = parse_gallery_meta(INIT, "").expect("meta");
        assert_eq!(meta.collection_id, "119166914");
        assert_eq!(meta.collection_key, "carolinasummerswing");
        assert_eq!(meta.gallery_slug, "sneakpeaks");
        assert_eq!(js_str_field(INIT, "collectionUrlKey").unwrap(), "carolinasummerswing");
        assert_eq!(js_int_field(INIT, "collectionId").unwrap(), "119166914");
    }

    #[test]
    fn slug_override_wins() {
        let meta = parse_gallery_meta(INIT, "awards").expect("meta");
        assert_eq!(meta.gallery_slug, "awards");
    }

    #[test]
    fn protocol_relative_urls_become_https() {
        assert_eq!(
            absolutize("//images.pixieset.com/x-xlarge.jpg"),
            "https://images.pixieset.com/x-xlarge.jpg"
        );
    }
}
