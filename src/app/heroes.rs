//! Homepage hero photos: optional Facebook album, else local files under `static/images/`.

use oauth2::reqwest;
use serde::Deserialize;
use std::path::Path;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;
use tracing::{info, warn};

const CACHE_TTL: Duration = Duration::from_secs(15 * 60);
const FACEBOOK_PAGE_LIMIT: usize = 3;

#[derive(Debug)]
pub struct HeroGallery {
    album_id: String,
    access_token: String,
    cache: Mutex<HeroCache>,
}

#[derive(Debug, Default)]
struct HeroCache {
    urls: Vec<String>,
    fetched_at: Option<Instant>,
}

#[derive(Debug, Deserialize)]
struct AlbumResponse {
    data: Vec<AlbumPhoto>,
    paging: Option<AlbumPaging>,
}

#[derive(Debug, Deserialize)]
struct AlbumPhoto {
    images: Option<Vec<AlbumImage>>,
}

#[derive(Debug, Deserialize)]
struct AlbumImage {
    source: String,
    width: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AlbumPaging {
    next: Option<String>,
}

impl HeroGallery {
    pub fn init() -> Self {
        let album_id = std::env::var("FACEBOOK_ALBUM_ID").unwrap_or_default();
        let access_token = std::env::var("FACEBOOK_ACCESS_TOKEN").unwrap_or_default();
        if album_id.is_empty() || access_token.is_empty() {
            info!("Facebook hero album disabled (set FACEBOOK_ALBUM_ID and FACEBOOK_ACCESS_TOKEN).");
        } else {
            info!("Facebook hero album enabled for album {album_id}");
        }
        Self {
            album_id,
            access_token,
            cache: Mutex::new(HeroCache::default()),
        }
    }

    pub async fn urls(&self, client: &reqwest::Client) -> Vec<String> {
        let mut cache = self.cache.lock().await;
        if let Some(fetched_at) = cache.fetched_at
            && fetched_at.elapsed() < CACHE_TTL
            && !cache.urls.is_empty()
        {
            return cache.urls.clone();
        }

        let mut urls = Vec::new();
        if !self.album_id.is_empty() && !self.access_token.is_empty() {
            match fetch_facebook_album(client, &self.album_id, &self.access_token).await {
                Ok(fb_urls) if !fb_urls.is_empty() => {
                    info!("Loaded {} Facebook album photos for the homepage hero", fb_urls.len());
                    urls = fb_urls;
                }
                Ok(_) => warn!("Facebook album returned no photos; using local images"),
                Err(err) => warn!(%err, "Facebook album fetch failed; using local images"),
            }
        }

        if urls.is_empty() {
            urls = local_hero_urls();
        }

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

async fn fetch_facebook_album(
    client: &reqwest::Client,
    album_id: &str,
    access_token: &str,
) -> Result<Vec<String>, String> {
    let mut url = format!("https://graph.facebook.com/v21.0/{album_id}/photos");
    let mut urls = Vec::new();
    let mut first_page = true;

    for _ in 0..FACEBOOK_PAGE_LIMIT {
        let mut req = client.get(&url);
        if first_page {
            req = req.query(&[
                ("fields", "images"),
                ("limit", "100"),
                ("access_token", access_token),
            ]);
            first_page = false;
        }
        let res = req.send().await.map_err(|err| err.to_string())?;
        if !res.status().is_success() {
            let status = res.status();
            let body = res.text().await.unwrap_or_default();
            return Err(format!("Graph API {status}: {body}"));
        }
        let parsed: AlbumResponse = res.json().await.map_err(|err| err.to_string())?;
        for photo in parsed.data {
            if let Some(source) = largest_image(photo.images) {
                urls.push(source);
            }
        }
        match parsed.paging.and_then(|p| p.next) {
            Some(next) if !next.is_empty() => url = next,
            _ => break,
        }
    }

    Ok(urls)
}

fn largest_image(images: Option<Vec<AlbumImage>>) -> Option<String> {
    images?
        .into_iter()
        .max_by_key(|img| img.width.unwrap_or(0))
        .map(|img| img.source)
}
