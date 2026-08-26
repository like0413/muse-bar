use std::io::Cursor;

use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};

use crate::platform::windows::{DwmGetColorizationColor, BOOL};

const MAX_ARTWORK_BYTES: u64 = 4 * 1024 * 1024;
const MAX_ARTWORK_DIMENSION: u32 = 4096;
const MAX_ARTWORK_DECODE_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_WINDOWS_ACCENT_COLOR: &str = "#0078D4";

/// 一次缩略图读取产生的前端图片和候选主色。
pub(crate) struct MediaArtwork {
    pub(crate) data_url: String,
    pub(crate) accent_color: Option<String>,
}

/// 用于统计相近颜色出现次数及其实际 RGB 均值。
#[derive(Clone, Copy, Default)]
struct ColorBucket {
    count: u32,
    red_sum: u64,
    green_sum: u64,
    blue_sum: u64,
}

/// 读取一次 SMTC 缩略图流，同时生成 WebView 图片和封面主色。
pub(crate) fn read_artwork(
    thumbnail: &IRandomAccessStreamReference,
) -> Result<Option<MediaArtwork>, String> {
    let stream = thumbnail
        .OpenReadAsync()
        .and_then(|operation| operation.get())
        .map_err(|error| format!("无法打开媒体封面流：{error}"))?;
    let size = stream
        .Size()
        .map_err(|error| format!("无法读取媒体封面大小：{error}"))?;

    if size == 0 {
        return Ok(None);
    }
    if size > MAX_ARTWORK_BYTES {
        return Err(format!(
            "媒体封面大小为 {size} 字节，超过 {MAX_ARTWORK_BYTES} 字节限制"
        ));
    }

    let byte_count = u32::try_from(size).map_err(|_| "媒体封面大小无法转换为 u32".to_owned())?;
    let input_stream = stream
        .GetInputStreamAt(0)
        .map_err(|error| format!("无法定位媒体封面流：{error}"))?;
    let reader = DataReader::CreateDataReader(&input_stream)
        .map_err(|error| format!("无法创建媒体封面读取器：{error}"))?;
    let loaded_count = reader
        .LoadAsync(byte_count)
        .and_then(|operation| operation.get())
        .map_err(|error| format!("无法载入媒体封面字节：{error}"))?;
    let mut bytes = vec![0; loaded_count as usize];
    reader
        .ReadBytes(&mut bytes)
        .map_err(|error| format!("无法复制媒体封面字节：{error}"))?;

    if bytes.is_empty() {
        return Ok(None);
    }

    let reported_content_type = stream
        .ContentType()
        .map(|content_type| content_type.to_string())
        .unwrap_or_default();
    let content_type = detect_artwork_content_type(&bytes, &reported_content_type);
    // 当前函数运行在专用媒体元数据线程中，图片解码不会阻塞 Tauri 命令或 WebView。
    // WebP 可能来自网页播放器，因此与常见的 JPEG/PNG 一并解码主色。
    // WebView 仍可直接展示少见的 BMP/GIF；主色提取失败时保留封面并回退系统强调色。
    let accent_color = match extract_dominant_color(&bytes) {
        Ok(color) => color,
        Err(error) => {
            log::warn!("无法从媒体封面提取主色，将使用系统强调色：{error}");
            None
        }
    };
    let encoded = BASE64_STANDARD.encode(&bytes);

    Ok(Some(MediaArtwork {
        data_url: format!("data:{content_type};base64,{encoded}"),
        accent_color,
    }))
}

/// 从缩小后的封面中选择出现频率高、且在深浅背景上都可辨识的颜色。
fn extract_dominant_color(bytes: &[u8]) -> Result<Option<String>, String> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|error| format!("无法识别媒体封面格式：{error}"))?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_ARTWORK_DIMENSION);
    limits.max_image_height = Some(MAX_ARTWORK_DIMENSION);
    limits.max_alloc = Some(MAX_ARTWORK_DECODE_BYTES);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|error| format!("媒体封面损坏或超过解码限制：{error}"))?
        .thumbnail(48, 48)
        .to_rgba8();
    let mut buckets = [ColorBucket::default(); 4096];

    for pixel in image.pixels() {
        let [red, green, blue, alpha] = pixel.0;
        if alpha < 128 {
            continue;
        }

        let index = ((usize::from(red) >> 4) << 8)
            | ((usize::from(green) >> 4) << 4)
            | (usize::from(blue) >> 4);
        let bucket = &mut buckets[index];
        bucket.count += 1;
        bucket.red_sum += u64::from(red);
        bucket.green_sum += u64::from(green);
        bucket.blue_sum += u64::from(blue);
    }

    let mut best: Option<(u64, u8, u8, u8)> = None;
    for bucket in buckets.into_iter().filter(|bucket| bucket.count > 0) {
        let count = u64::from(bucket.count);
        let red = (bucket.red_sum / count) as u8;
        let green = (bucket.green_sum / count) as u8;
        let blue = (bucket.blue_sum / count) as u8;
        let maximum = red.max(green).max(blue);
        let minimum = red.min(green).min(blue);
        let saturation = maximum - minimum;

        // 低饱和颜色容易与任务栏融为一体；同时要求候选色在典型深浅背景上均有辨识度。
        if saturation < 24 || !has_sufficient_taskbar_contrast(red, green, blue) {
            continue;
        }

        // 在出现频率仍占主导的前提下，提高鲜艳候选色的权重，避免总是选中大面积灰暗背景。
        let score = count * (u64::from(saturation) * 2 + 32);
        if best
            .as_ref()
            .map_or(true, |(best_score, ..)| score > *best_score)
        {
            best = Some((score, red, green, blue));
        }
    }

    Ok(best.map(|(_, red, green, blue)| {
        let (red, green, blue) = enhance_color_saturation(red, green, blue);
        format!("#{red:02X}{green:02X}{blue:02X}")
    }))
}

/// 温和提高颜色饱和度并保持最高通道亮度；增强后对比度不足时保留原色。
fn enhance_color_saturation(red: u8, green: u8, blue: u8) -> (u8, u8, u8) {
    let maximum = red.max(green).max(blue);
    let minimum = red.min(green).min(blue);
    let difference = maximum - minimum;
    if maximum == 0 || difference == 0 {
        return (red, green, blue);
    }

    let maximum = f64::from(maximum);
    let saturation = f64::from(difference) / maximum;
    let enhanced_saturation = (saturation * 1.15 + 0.05).min(1.0);
    let expansion = enhanced_saturation / saturation;

    // 以最亮通道为锚点向外拉开其余通道，保持色相和视觉亮度基本稳定。
    let enhance_channel = |channel: u8| {
        (maximum - (maximum - f64::from(channel)) * expansion)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    let enhanced = (
        enhance_channel(red),
        enhance_channel(green),
        enhance_channel(blue),
    );

    if has_sufficient_taskbar_contrast(enhanced.0, enhanced.1, enhanced.2) {
        enhanced
    } else {
        (red, green, blue)
    }
}

/// 判断颜色相对典型深色与浅色任务栏背景是否都具有最低辨识度。
fn has_sufficient_taskbar_contrast(red: u8, green: u8, blue: u8) -> bool {
    let luminance = relative_luminance(red, green, blue);
    let light_background = relative_luminance(245, 245, 245);
    let dark_background = relative_luminance(32, 32, 32);
    contrast_ratio(luminance, light_background) >= 1.6
        && contrast_ratio(luminance, dark_background) >= 1.6
}

/// 将 sRGB 颜色转换为用于对比度计算的相对亮度。
fn relative_luminance(red: u8, green: u8, blue: u8) -> f64 {
    /// 将单个 sRGB 通道转换为线性光通道。
    fn linearize(channel: u8) -> f64 {
        let channel = f64::from(channel) / 255.0;
        if channel <= 0.04045 {
            channel / 12.92
        } else {
            ((channel + 0.055) / 1.055).powf(2.4)
        }
    }

    0.2126 * linearize(red) + 0.7152 * linearize(green) + 0.0722 * linearize(blue)
}

/// 计算两个相对亮度之间的 WCAG 对比度。
fn contrast_ratio(first: f64, second: f64) -> f64 {
    let lighter = first.max(second);
    let darker = first.min(second);
    (lighter + 0.05) / (darker + 0.05)
}

/// 读取 Windows 当前强调色；系统 API 不可用时采用 Windows 默认蓝色。
pub(crate) fn read_windows_accent_color() -> String {
    let mut colorization = 0_u32;
    let mut opaque_blend = BOOL::default();
    if unsafe { DwmGetColorizationColor(&mut colorization, &mut opaque_blend) }.is_err() {
        return DEFAULT_WINDOWS_ACCENT_COLOR.to_owned();
    }

    format!("#{:06X}", colorization & 0x00FF_FFFF)
}

/// 根据图片文件头确定 WebView 使用的 MIME；无法识别时才采用播放器上报的首个类型。
fn detect_artwork_content_type(bytes: &[u8], reported_content_type: &str) -> String {
    let detected = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        Some("image/png")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if bytes.starts_with(b"BM") {
        Some("image/bmp")
    } else {
        None
    };

    if let Some(content_type) = detected {
        return content_type.to_owned();
    }

    // 有些播放器会返回逗号分隔的扩展名列表。data URL 只接受单一 MIME，
    // 因此最多采用第一个不带参数的 image/* 类型。
    let reported = reported_content_type
        .split([',', ';'])
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if reported.starts_with("image/") {
        reported
    } else {
        "image/jpeg".to_owned()
    }
}
