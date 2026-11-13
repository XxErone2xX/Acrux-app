//! OCR of the requested captured pixels, never the user's display.
use crate::{
    automation::Word,
    vision::{Frame, Rect, Region},
};
use std::{
    cell::RefCell,
    fs,
    io::Write,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Retry {
    generation: i32,
    geometry: (usize, usize),
    coverage: Region,
    regions: Vec<Region>,
}
thread_local! {static RETRY:RefCell<Option<Retry>>=const{RefCell::new(None)};}
#[derive(Clone, Copy)]
struct Patch {
    source: Region,
    atlas: Region,
}
pub(super) fn invalidate() {
    RETRY.with(|r| r.borrow_mut().take());
}

pub(super) fn read(
    image: &super::Image,
    generation: i32,
    captured_at: f64,
) -> Result<Vec<Word>, String> {
    let frame = image
        .frame()
        .ok_or("OCR 화면 데이터의 길이가 잘못됐습니다.")?;
    let r = frame.coverage();
    // A slow full pass can locate text, but none of its strings drive input.
    // Its boxes are a one-use scan hint for OCR of NEW pixels of the same
    // unchanged execution. Any input invalidates this hint.
    let hint = RETRY
        .with(|cached| cached.borrow_mut().take())
        .filter(|retry| {
            retry.generation == generation
                && retry.geometry == (frame.width, frame.height)
                && retry.coverage == r
        });
    let (patches, width, height) = layout(r, hint.as_ref().map(|h| h.regions.as_slice()));
    let dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let image_path = dir.path().join("capture.ppm");
    let output = dir.path().join("text");
    let mut file = fs::File::create(&image_path).map_err(|e| e.to_string())?;
    write!(file, "P6\n{} {}\n255\n", width, height).map_err(|e| e.to_string())?;
    let mut row = vec![0; width * 3];
    for y in 0..height {
        row.fill(0);
        for patch in patches
            .iter()
            .filter(|p| y >= p.atlas.y && y < p.atlas.y + p.atlas.height)
        {
            let source_y = patch.source.y + y - patch.atlas.y;
            let start =
                (source_y - r.y) * image.stride + (patch.source.x - r.x) * image.pixel_stride;
            let pixels = image
                .data
                .get(start..start + patch.source.width * image.pixel_stride)
                .ok_or("OCR 영역 픽셀이 없습니다.")?;
            let output = &mut row[patch.atlas.x * 3..(patch.atlas.x + patch.source.width) * 3];
            if image.pixel_stride == 3 && !image.bgra {
                output.copy_from_slice(pixels);
            } else {
                for (dst, src) in output
                    .chunks_exact_mut(3)
                    .zip(pixels.chunks_exact(image.pixel_stride))
                {
                    if image.bgra {
                        dst.copy_from_slice(&[src[2], src[1], src[0]]);
                    } else {
                        dst.copy_from_slice(&src[..3]);
                    }
                }
            }
        }
        file.write_all(&row).map_err(|e| e.to_string())?;
    }
    drop(file);
    // English is always available in the distributed Linux package. Additional
    // installed language data is selected explicitly by the user's UI language.
    let lang = crate::runtime().macro_config()["lang"]
        .as_str()
        .unwrap_or("en")
        .to_owned();
    let lang = match lang.as_str() {
        "ko" => "eng+kor",
        "ja" => "eng+jpn",
        _ => "eng",
    };
    let mut child = Command::new("tesseract")
        .arg(&image_path)
        .arg(&output)
        .args([
            "-l",
            lang,
            "--oem",
            "1",
            "--psm",
            "11",
            "-c",
            "tessedit_create_tsv=1",
        ])
        .env("OMP_THREAD_LIMIT", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| "Linux 화면 읽기 패키지(tesseract-ocr 및 언어 데이터)를 설치하세요.")?;
    let end = Instant::now() + Duration::from_secs(3);
    loop {
        if super::SESSION_STOP.load(std::sync::atomic::Ordering::Acquire)
            || crate::runtime().generation != generation
        {
            let _ = child.kill();
            let _ = child.wait();
            return Err("현재 실행이 바뀌어 화면 글자 읽기를 취소했습니다.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    return Err(format!(
                        "화면 글자 읽기에 실패했습니다. {lang} 언어 데이터를 확인하세요."
                    ));
                }
                break;
            }
            Ok(None) if Instant::now() < end => thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("화면 글자 읽기 시간이 초과됐습니다.".into());
            }
        }
    }
    let path = output.with_extension("tsv");
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2 * 1024 * 1024 {
        return Err("OCR 결과가 너무 큽니다.".into());
    }
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let words = parse(&text, &patches, frame.width, frame.height)?;
    if hint.is_none() && crate::now() > captured_at + 0.300 {
        if let Some(regions) = retry_regions(&frame, &words) {
            RETRY.with(|cached| {
                *cached.borrow_mut() = Some(Retry {
                    generation,
                    geometry: (frame.width, frame.height),
                    coverage: r,
                    regions,
                })
            });
        }
    }
    Ok(words)
}

fn parse(text: &str, patches: &[Patch], w: usize, h: usize) -> Result<Vec<Word>, String> {
    use std::collections::BTreeMap;
    let mut lines: BTreeMap<(usize, u32, u32, u32, u32), Word> = BTreeMap::new();
    for line in text.lines().skip(1) {
        let a: Vec<_> = line.splitn(12, '\t').collect();
        if a.len() != 12 || a[0] != "5" || a[11].trim().is_empty() {
            continue;
        }
        let n = |i: usize| a[i].parse::<usize>().ok();
        let (Some(left), Some(top), Some(width), Some(height)) = (n(6), n(7), n(8), n(9)) else {
            continue;
        };
        if width == 0 || height == 0 || a[10].parse::<f64>().unwrap_or(-1.) < 15. {
            continue;
        }
        let Some((patch, rect)) = map_box(
            patches,
            Region {
                x: left,
                y: top,
                width,
                height,
            },
            (w, h),
        ) else {
            continue;
        };
        let key = (
            patch,
            n(1).unwrap_or(0) as u32,
            n(2).unwrap_or(0) as u32,
            n(3).unwrap_or(0) as u32,
            n(4).unwrap_or(0) as u32,
        );
        lines
            .entry(key)
            .and_modify(|word| {
                if word.text.len() < 4096 {
                    word.text.push(' ');
                    word.text.push_str(a[11].trim());
                    word.rect = Rect(
                        word.rect.0.min(rect.0),
                        word.rect.1.min(rect.1),
                        word.rect.2.max(rect.2),
                        word.rect.3.max(rect.3),
                    );
                }
            })
            .or_insert(Word {
                text: a[11].trim().chars().take(512).collect(),
                rect,
            });
        if lines.len() > 1024 {
            return Err("OCR 글자 수가 상한을 넘었습니다.".into());
        }
    }
    let mut words: Vec<_> = lines.into_values().collect();
    words.sort_by(|a, b| {
        a.rect
            .1
            .total_cmp(&b.rect.1)
            .then_with(|| a.rect.0.total_cmp(&b.rect.0))
    });
    Ok(words)
}

fn map_box(patches: &[Patch], r: Region, geometry: (usize, usize)) -> Option<(usize, Rect)> {
    let right = r.x.checked_add(r.width)?;
    let bottom = r.y.checked_add(r.height)?;
    let (index, p) = patches.iter().enumerate().find(|(_, p)| {
        r.x >= p.atlas.x
            && r.y >= p.atlas.y
            && right <= p.atlas.x + p.atlas.width
            && bottom <= p.atlas.y + p.atlas.height
    })?;
    let (x, y) = (p.source.x + r.x - p.atlas.x, p.source.y + r.y - p.atlas.y);
    Some((
        index,
        Rect(
            x as f64 / geometry.0 as f64,
            y as f64 / geometry.1 as f64,
            (x + r.width) as f64 / geometry.0 as f64,
            (y + r.height) as f64 / geometry.1 as f64,
        ),
    ))
}
fn layout(coverage: Region, regions: Option<&[Region]>) -> (Vec<Patch>, usize, usize) {
    let full = || {
        (
            vec![Patch {
                source: coverage,
                atlas: Region::full(coverage.width, coverage.height),
            }],
            coverage.width,
            coverage.height,
        )
    };
    let Some(regions) = regions.filter(|r| !r.is_empty() && r.len() <= 256) else {
        return full();
    };
    let width = regions
        .iter()
        .map(|r| r.width + 16)
        .max()
        .unwrap()
        .max(coverage.width.min(960));
    let (mut x, mut y, mut row_height) = (8, 8, 0);
    let mut patches = Vec::new();
    for &source in regions {
        if x + source.width + 8 > width {
            x = 8;
            y += row_height + 12;
            row_height = 0;
        }
        patches.push(Patch {
            source,
            atlas: Region {
                x,
                y,
                width: source.width,
                height: source.height,
            },
        });
        x += source.width + 16;
        row_height = row_height.max(source.height);
    }
    let height = y + row_height + 8;
    if width
        .checked_mul(height)
        .is_none_or(|area| area >= coverage.width * coverage.height * 3 / 4)
    {
        return full();
    }
    (patches, width, height)
}
fn retry_regions(frame: &Frame<'_>, words: &[Word]) -> Option<Vec<Region>> {
    if words.is_empty() || words.len() > 256 {
        return None;
    }
    let coverage = frame.coverage();
    let mut regions: Vec<Region> = Vec::new();
    for word in words {
        let Some(r) = word.rect.region((frame.width, frame.height)) else {
            continue;
        };
        let x = r.x.saturating_sub(8).max(coverage.x);
        let y = r.y.saturating_sub(6).max(coverage.y);
        let right = (r.x + r.width + 8).min(coverage.x + coverage.width);
        let bottom = (r.y + r.height + 6).min(coverage.y + coverage.height);
        if right <= x || bottom <= y {
            continue;
        }
        let mut next = Region {
            x,
            y,
            width: right - x,
            height: bottom - y,
        };
        while let Some(index) = regions.iter().position(|p| {
            next.x < p.x + p.width
                && p.x < next.x + next.width
                && next.y < p.y + p.height
                && p.y < next.y + next.height
        }) {
            let p = regions.swap_remove(index);
            let right = (p.x + p.width).max(next.x + next.width);
            let bottom = (p.y + p.height).max(next.y + next.height);
            let x = p.x.min(next.x);
            let y = p.y.min(next.y);
            next = Region {
                x,
                y,
                width: right - x,
                height: bottom - y,
            };
        }
        regions.push(next);
    }
    regions.sort_by_key(|r| (r.y, r.x));
    (!regions.is_empty()).then_some(regions)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(path: &str) -> super::super::Image {
        let bytes = std::fs::read(path).unwrap();
        let mut parts = bytes.splitn(4, |&b| b == b'\n');
        assert_eq!(parts.next().unwrap(), b"P6");
        let dimensions: Vec<usize> = std::str::from_utf8(parts.next().unwrap())
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(parts.next().unwrap(), b"255");
        let (w, h) = (dimensions[0], dimensions[1]);
        let pixels = parts.next().unwrap();
        super::super::Image {
            data: pixels
                .chunks_exact(3)
                .flat_map(|p| [p[2], p[1], p[0], 0])
                .collect(),
            width: w,
            height: h,
            stride: w * 4,
            pixel_stride: 4,
            bgra: true,
            region: None,
        }
    }
    #[test]
    #[ignore = "requires two synthetic P6 fixtures in ACRUX_OCR_FIXTURE / ACRUX_OCR_CHANGED"]
    fn retry_boxes_read_changed_pixels_instead_of_reusing_discarded_text() {
        let first = fixture(&std::env::var("ACRUX_OCR_FIXTURE").unwrap());
        let second = fixture(&std::env::var("ACRUX_OCR_CHANGED").unwrap());
        invalidate();
        crate::runtime().macro_settings = serde_json::json!({"lang":"en"});
        crate::runtime().generation = 777;
        let words = read(&first, 777, crate::now() - 1.).unwrap();
        let contains = |words: &[Word], s: &str| {
            words
                .iter()
                .any(|w| w.text.to_lowercase().replace(' ', "").contains(s))
        };
        assert!(contains(&words, "captainflarg"));
        assert!(RETRY.with(|hint| hint.borrow().is_some()));
        let words = read(&second, 777, crate::now()).unwrap();
        assert!(contains(&words, "captainfloyd"));
        assert!(!contains(&words, "captainflarg"));
        assert!(
            RETRY.with(|hint| hint.borrow().is_none()),
            "a hint is consumed once, then the next scan covers new UI again"
        );
    }
    #[test]
    fn packed_text_reads_map_to_real_viewport_and_do_not_merge_unrelated_patches() {
        let regions = [
            Region {
                x: 100,
                y: 100,
                width: 200,
                height: 30,
            },
            Region {
                x: 1300,
                y: 900,
                width: 200,
                height: 30,
            },
        ];
        let (patches, w, h) = layout(Region::full(1920, 1080), Some(&regions));
        assert!(w * h < 1920 * 1080 * 3 / 4);
        let header="level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n";
        let mut tsv = String::from(header);
        for (i, p) in patches.iter().enumerate() {
            tsv.push_str(&format!(
                "5\t1\t1\t1\t1\t{}\t{}\t{}\t40\t12\t91\t{}\n",
                i + 1,
                p.atlas.x + 4,
                p.atlas.y + 3,
                if i == 0 { "Player" } else { "Search" }
            ));
        }
        let words = parse(&tsv, &patches, 1920, 1080).unwrap();
        assert_eq!(words.len(), 2);
        assert_eq!(words[0].text, "Player");
        assert_eq!(words[1].text, "Search");
        assert!((words[1].rect.0 - 1304. / 1920.).abs() < 1e-9);
        assert!((words[1].rect.1 - 903. / 1080.).abs() < 1e-9);
        assert!(map_box(
            &patches,
            Region {
                x: patches[0].atlas.x + 195,
                y: patches[0].atlas.y,
                width: 30,
                height: 12
            },
            (1920, 1080)
        )
        .is_none());
        let (_, fw, fh) = layout(Region::full(1920, 1080), Some(&[Region::full(1920, 1080)]));
        assert_eq!((fw, fh), (1920, 1080));
    }
    #[test]
    #[ignore = "requires a synthetic P6 fixture in ACRUX_OCR_FIXTURE and eng/kor/jpn trained data"]
    fn synthetic_ocr_latency_and_retry_use_new_pixels() {
        let image = fixture(&std::env::var("ACRUX_OCR_FIXTURE").unwrap());
        for (index, lang) in ["en", "ko", "ja"].into_iter().enumerate() {
            invalidate();
            crate::runtime().macro_settings = serde_json::json!({"lang":lang});
            crate::runtime().generation = index as i32;
            for pass in 0..3 {
                let cropped = RETRY.with(|hint| hint.borrow().is_some());
                let captured = crate::now();
                // Include an intentional capture/copy delay in the age budget;
                // the OCR runtime below is measured separately from this delay.
                thread::sleep(Duration::from_millis(40));
                let began = crate::now();
                let words = read(&image, index as i32, captured).unwrap();
                let elapsed = crate::now() - captured;
                println!(
                    "{lang} pass {pass} crop={cropped}: age {:.0}ms, read {:.0}ms, {} text lines",
                    elapsed * 1000.,
                    (crate::now() - began) * 1000.,
                    words.len()
                );
                assert!(words.iter().any(|w| w
                    .text
                    .to_lowercase()
                    .replace(' ', "")
                    .contains("captainflarg")));
            }
        }
    }
    #[test]
    fn tsv_lines_keep_full_viewport_coordinates_and_ignore_noise() {
        let text="level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t10\t20\t30\t12\t91\tOpen\n5\t1\t1\t1\t1\t2\t44\t20\t40\t12\t88\tRecipe\n5\t1\t2\t1\t1\t1\t1\t1\t2\t2\t1\tnoise";
        let out = super::parse(
            text,
            &[super::Patch {
                source: super::Region {
                    x: 100,
                    y: 50,
                    width: 900,
                    height: 450,
                },
                atlas: super::Region::full(900, 450),
            }],
            1000,
            500,
        )
        .unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "Open Recipe");
        assert_eq!(out[0].rect.0, 0.11);
        assert_eq!(out[0].rect.3, 0.164);
    }
}
