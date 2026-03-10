// src/media.rs
use tokio::io::AsyncWriteExt;
use futures_util::StreamExt;

pub async fn download_image(
    client: &reqwest::Client,
    url: &str,
    output_path: &std::path::PathBuf,
) -> anyhow::Result<()> {
    let response = client.get(url).send().await?;
    let mut file = tokio::fs::File::create(output_path).await?;

    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
    }

    Ok(())
}

pub fn make_media_path(
    output_dir: &str,
    sheet: &str,
    id: &str,
    media_type: &crate::config::MediaType,
    index: usize,
    url: &str,
) -> std::path::PathBuf {
    let ext = url
        .split('?')
        .next()
        .unwrap_or(url)
        .rsplit('.')
        .next()
        .unwrap_or("jpg");

    let filename = format!("{index}.{ext}");

    let media_dir = match media_type {
        crate::config::MediaType::Image => "image",
        crate::config::MediaType::Video => "video",
        crate::config::MediaType::Other => "other",
    };

    std::path::Path::new(output_dir)
        .join(sheet)
        .join(id)
        .join(media_dir)
        .join(filename)
}