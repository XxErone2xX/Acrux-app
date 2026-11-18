//! Read only the requested game region and return coordinates in the original frame.
use crate::vision::{Frame, Rect};
use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

const MAX_PIXELS: usize = 8_388_608;
const MAX_WORDS: usize = 1024;
const MAX_TEXT: usize = 512;
const TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Clone, Debug, Deserialize, Serialize)]
/// One complete observed text line, using the union of its actual word boxes.
pub(crate) struct Word {
    pub text: String,
    pub rect: Rect,
}

struct Crop {
    left: usize,
    top: usize,
    width: usize,
    height: usize,
    content_width: usize,
    content_height: usize,
    padding: usize,
    source_width: usize,
    source_height: usize,
    output_width: usize,
    output_height: usize,
    frame_width: usize,
    frame_height: usize,
}

impl Crop {
    fn new(frame: &Frame<'_>, rect: Rect) -> Result<Self, String> {
        Self::with_options(frame, rect, None, 0)
    }

    fn with_options(
        frame: &Frame<'_>,
        rect: Rect,
        scale: Option<u32>,
        padding: u32,
    ) -> Result<Self, String> {
        let values = [rect.0, rect.1, rect.2, rect.3];
        if frame.width == 0
            || frame.height == 0
            || frame.width > 16384
            || frame.height > 16384
            || values
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || rect.0 >= rect.2
            || rect.1 >= rect.3
        {
            return Err("OCR 영역이 게임 화면 안의 유효한 사각형이어야 합니다.".into());
        }
        let left = (rect.0 * frame.width as f64).floor() as usize;
        let top = (rect.1 * frame.height as f64).floor() as usize;
        let right = ((rect.2 * frame.width as f64).ceil() as usize).min(frame.width);
        let bottom = ((rect.3 * frame.height as f64).ceil() as usize).min(frame.height);
        let width = right.saturating_sub(left);
        let height = bottom.saturating_sub(top);
        if width == 0 || height == 0 || width.checked_mul(height).is_none_or(|n| n > MAX_PIXELS) {
            return Err("OCR 영역이 비어 있거나 너무 큽니다.".into());
        }
        // Explicit digit reads use main's caller-selected scale and dark border.
        let scale = scale.unwrap_or(if width * height < 200 * 60 { 2 } else { 1 }) as usize;
        let padding = padding as usize;
        if !(1..=8).contains(&scale) || padding > 256 {
            return Err("OCR 확대·여백의 범위가 올바르지 않습니다.".into());
        }
        let content_width = width * scale;
        let content_height = height * scale;
        let output_width = content_width + padding * 2;
        let output_height = content_height + padding * 2;
        if output_width
            .checked_mul(output_height)
            .is_none_or(|n| n > MAX_PIXELS)
        {
            return Err("확대한 OCR 영역이 너무 큽니다.".into());
        }
        Ok(Self {
            left,
            top,
            width,
            height,
            content_width,
            content_height,
            padding,
            source_width: output_width,
            source_height: output_height,
            output_width,
            output_height,
            frame_width: frame.width,
            frame_height: frame.height,
        })
    }

    fn pixels<'a>(&'a self, frame: &'a Frame<'_>) -> impl Iterator<Item = [u8; 3]> + 'a {
        (0..self.output_height).flat_map(move |y| {
            (0..self.output_width).map(move |x| {
                let x = x * self.source_width / self.output_width;
                let y = y * self.source_height / self.output_height;
                if x < self.padding
                    || y < self.padding
                    || x >= self.padding + self.content_width
                    || y >= self.padding + self.content_height
                {
                    return [30, 30, 30];
                }
                frame.pixel(
                    self.left + (x - self.padding) * self.width / self.content_width,
                    self.top + (y - self.padding) * self.height / self.content_height,
                )
            })
        })
    }

    fn word(&self, text: &str, bounds: [f64; 4]) -> Option<Word> {
        let text = text.trim();
        let [x, y, width, height] = bounds;
        if text.is_empty()
            || text.len() > MAX_TEXT
            || text.chars().any(char::is_control)
            || bounds.iter().any(|v| !v.is_finite())
            || x < 0.
            || y < 0.
            || width <= 0.
            || height <= 0.
            || x + width > self.output_width as f64
            || y + height > self.output_height as f64
        {
            return None;
        }
        let source_x = self.source_width as f64 / self.output_width as f64;
        let source_y = self.source_height as f64 / self.output_height as f64;
        let x0 = (x * source_x - self.padding as f64).max(0.);
        let y0 = (y * source_y - self.padding as f64).max(0.);
        let x1 = ((x + width) * source_x - self.padding as f64).min(self.content_width as f64);
        let y1 = ((y + height) * source_y - self.padding as f64).min(self.content_height as f64);
        if x0 >= x1 || y0 >= y1 {
            return None;
        }
        let scale_x = self.width as f64 / self.content_width as f64;
        let scale_y = self.height as f64 / self.content_height as f64;
        Some(Word {
            text: text.into(),
            rect: Rect(
                (self.left as f64 + x0 * scale_x) / self.frame_width as f64,
                (self.top as f64 + y0 * scale_y) / self.frame_height as f64,
                (self.left as f64 + x1 * scale_x) / self.frame_width as f64,
                (self.top as f64 + y1 * scale_y) / self.frame_height as f64,
            ),
        })
    }
}

pub(crate) fn read(frame: &Frame<'_>, region: Rect) -> Result<Vec<Word>, String> {
    let crop = Crop::new(frame, region)?;
    read_crop(frame, crop)
}

/// Apply the caller's scale, then a dark border measured in scaled pixels.
/// Returned boxes still refer to the original unpadded game region.
pub(crate) fn read_with_options(
    frame: &Frame<'_>,
    region: Rect,
    scale: u32,
    pad: u32,
) -> Result<Vec<Word>, String> {
    let crop = Crop::with_options(frame, region, Some(scale), pad)?;
    read_crop(frame, crop)
}

fn read_crop(frame: &Frame<'_>, crop: Crop) -> Result<Vec<Word>, String> {
    #[cfg(target_os = "linux")]
    {
        linux::read(frame, &crop)
    }
    #[cfg(target_os = "windows")]
    {
        windows_ocr::read(frame, crop)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = crop;
        Err("이 환경에는 OCR 엔진이 연결되지 않았습니다.".into())
    }
}

#[cfg(any(target_os = "linux", test))]
fn ppm(frame: &Frame<'_>, crop: &Crop) -> Vec<u8> {
    let mut data = format!("P6\n{} {}\n255\n", crop.output_width, crop.output_height).into_bytes();
    data.reserve(crop.output_width * crop.output_height * 3);
    for pixel in crop.pixels(frame) {
        data.extend_from_slice(&pixel);
    }
    data
}

fn line_box(words: &[Word], observed_text: Option<&str>) -> Option<Word> {
    let first = words.first()?;
    let text = observed_text.map(str::to_owned).unwrap_or_else(|| {
        words
            .iter()
            .map(|word| word.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    });
    let text = text.trim();
    if text.is_empty() || text.len() > MAX_TEXT || text.chars().any(char::is_control) {
        return None;
    }
    let rect = words.iter().skip(1).fold(first.rect, |all, word| {
        Rect(
            all.0.min(word.rect.0),
            all.1.min(word.rect.1),
            all.2.max(word.rect.2),
            all.3.max(word.rect.3),
        )
    });
    Some(Word {
        text: text.into(),
        rect,
    })
}

#[cfg(any(target_os = "linux", test))]
fn parse_tsv(text: &str, crop: &Crop) -> Vec<Word> {
    let mut lines: Vec<Vec<Word>> = Vec::new();
    let mut line_indices = std::collections::BTreeMap::new();
    let mut count = 0;
    for row in text.lines() {
        let columns: Vec<_> = row.splitn(12, '\t').collect();
        if columns.len() != 12 || columns[0] != "5" {
            continue;
        }
        if !columns[10]
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && (0.0..=100.0).contains(&value))
        {
            continue;
        }
        let Some(key) = columns[1..5]
            .iter()
            .map(|value| value.parse::<u32>().ok())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Some(bounds) = columns[6..10]
            .iter()
            .map(|value| value.parse::<f64>().ok())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let Some(word) = crop.word(columns[11], [bounds[0], bounds[1], bounds[2], bounds[3]])
        else {
            continue;
        };
        let index = *line_indices.entry(key).or_insert_with(|| {
            lines.push(Vec::new());
            lines.len() - 1
        });
        lines[index].push(word);
        count += 1;
        if count == MAX_WORDS {
            break;
        }
    }
    lines
        .into_iter()
        .filter_map(|mut words| {
            words.sort_by(|a, b| a.rect.0.total_cmp(&b.rect.0));
            line_box(&words, None)
        })
        .collect()
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use std::{
        io::{Read, Write},
        process::{Command, Stdio},
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc, LazyLock,
        },
        thread,
    };

    const MAX_OUTPUT: usize = 1024 * 1024;
    const MAX_ERROR: usize = 16 * 1024;
    // Honour an explicit installation first, then reuse the app's standard
    // language data. This never reads a model from the current directory.
    static DATA_DIRECTORY: LazyLock<Option<std::path::PathBuf>> = LazyLock::new(|| {
        if let Some(directory) = std::env::var_os("TESSDATA_PREFIX").map(std::path::PathBuf::from) {
            return Some(directory);
        }
        let config = std::env::var_os("XDG_CONFIG_HOME")
            .map(std::path::PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join(".config"))
            })?;
        let directory = config.join("com.acrux.next/ocr/tessdata-fast-4.1.0");
        ["eng", "kor", "jpn"]
            .iter()
            .any(|language| directory.join(format!("{language}.traineddata")).is_file())
            .then_some(directory)
    });
    static LANGUAGES: LazyLock<Result<String, String>> = LazyLock::new(|| {
        let output = run(&["--list-langs"], None)?;
        let installed =
            std::str::from_utf8(&output).map_err(|_| "OCR 언어 목록을 읽지 못했습니다.")?;
        let selected: Vec<_> = ["eng", "kor", "jpn"]
            .into_iter()
            .filter(|lang| installed.lines().any(|line| line.trim() == *lang))
            .collect();
        if selected.is_empty() {
            Err("Tesseract OCR 언어팩이 없습니다. 영어·한국어·일본어 언어팩을 설치하세요.".into())
        } else {
            Ok(selected.join("+"))
        }
    });

    pub(super) fn read(frame: &Frame<'_>, crop: &Crop) -> Result<Vec<Word>, String> {
        let languages = LANGUAGES.as_ref().map_err(Clone::clone)?;
        let output = run(
            &[
                "stdin", "stdout", "-l", languages, "--psm", "11", "--dpi", "96", "tsv",
            ],
            Some(ppm(frame, crop)),
        )?;
        let text = std::str::from_utf8(&output).map_err(|_| "OCR 결과가 UTF-8이 아닙니다.")?;
        Ok(parse_tsv(text, crop))
    }

    fn run(args: &[&str], input: Option<Vec<u8>>) -> Result<Vec<u8>, String> {
        let mut command = Command::new("tesseract");
        if let Some(directory) = DATA_DIRECTORY.as_ref() {
            command.arg("--tessdata-dir").arg(directory);
        }
        let mut child = command
            .args(args)
            .env("OMP_THREAD_LIMIT", "2")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| format!("Tesseract OCR을 시작하지 못했습니다: {error}"))?;
        let started = Instant::now();
        let overflow = Arc::new(AtomicBool::new(false));
        let stdout = child.stdout.take().ok_or("OCR 출력 연결이 없습니다.")?;
        let stderr = child
            .stderr
            .take()
            .ok_or("OCR 오류 출력 연결이 없습니다.")?;
        let output_flag = overflow.clone();
        let output = thread::spawn(move || bounded_read(stdout, MAX_OUTPUT, &output_flag));
        let error_flag = overflow.clone();
        let errors = thread::spawn(move || bounded_read(stderr, MAX_ERROR, &error_flag));
        let mut stdin = child.stdin.take().ok_or("OCR 입력 연결이 없습니다.")?;
        let writer = thread::spawn(move || {
            if let Some(input) = input {
                stdin.write_all(&input)?;
            }
            Ok::<_, std::io::Error>(())
        });
        let outcome = loop {
            if overflow.load(Ordering::Acquire) {
                let _ = child.kill();
                let _ = child.wait();
                break Err("OCR 출력 크기가 제한을 넘었습니다.".to_owned());
            }
            if started.elapsed() >= TIMEOUT {
                let _ = child.kill();
                let _ = child.wait();
                break Err("OCR 응답 시간이 초과됐습니다.".to_owned());
            }
            match child.try_wait() {
                Ok(Some(status)) => break Ok(status),
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break Err(format!("OCR 실행 상태를 읽지 못했습니다: {error}"));
                }
            }
        };
        let written = writer.join().map_err(|_| "OCR 입력 작업이 중단됐습니다.")?;
        let output = output
            .join()
            .map_err(|_| "OCR 출력 작업이 중단됐습니다.")?
            .map_err(|error| error.to_string())?;
        let errors = errors
            .join()
            .map_err(|_| "OCR 오류 확인 작업이 중단됐습니다.")?
            .map_err(|error| error.to_string())?;
        let status = outcome?;
        if !status.success() {
            let details = String::from_utf8_lossy(&errors);
            return Err(format!("Tesseract OCR이 실패했습니다: {}", details.trim()));
        }
        written.map_err(|error| format!("OCR 화면을 보내지 못했습니다: {error}"))?;
        Ok(output)
    }

    fn bounded_read(
        mut stream: impl Read,
        limit: usize,
        overflow: &AtomicBool,
    ) -> std::io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        stream
            .by_ref()
            .take(limit as u64 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > limit {
            overflow.store(true, Ordering::Release);
            return Err(std::io::Error::other("OCR output exceeded limit"));
        }
        Ok(bytes)
    }
}

#[cfg(target_os = "windows")]
mod windows_ocr {
    use super::*;
    use std::thread;
    use windows::{
        Graphics::Imaging::{BitmapPixelFormat, SoftwareBitmap},
        Media::Ocr::OcrEngine,
        Storage::Streams::DataWriter,
        Win32::System::WinRT::{RoInitialize, RoUninitialize, RO_INIT_MULTITHREADED},
    };

    pub(super) fn read(frame: &Frame<'_>, mut crop: Crop) -> Result<Vec<Word>, String> {
        struct Apartment;
        impl Drop for Apartment {
            fn drop(&mut self) {
                unsafe {
                    RoUninitialize();
                }
            }
        }
        let apartment = match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
            Ok(()) => Some(Apartment),
            Err(error) if error.code().0 as u32 == 0x80010106 => None,
            Err(error) => return Err(format!("Windows OCR을 초기화하지 못했습니다: {error}")),
        };
        let result = (|| -> windows::core::Result<Vec<Word>> {
            let limit = OcrEngine::MaxImageDimension()?.max(1) as usize;
            let reduction = crop.output_width.max(crop.output_height).div_ceil(limit);
            if reduction > 1 {
                crop.output_width = (crop.output_width / reduction).max(1);
                crop.output_height = (crop.output_height / reduction).max(1);
            }
            let mut pixels = Vec::with_capacity(crop.output_width * crop.output_height * 4);
            for [r, g, b] in crop.pixels(frame) {
                pixels.extend_from_slice(&[b, g, r, 255]);
            }
            let writer = DataWriter::new()?;
            writer.WriteBytes(&pixels)?;
            let bitmap = SoftwareBitmap::CreateCopyFromBuffer(
                &writer.DetachBuffer()?,
                BitmapPixelFormat::Bgra8,
                crop.output_width as i32,
                crop.output_height as i32,
            )?;
            let engine = OcrEngine::TryCreateFromUserProfileLanguages()?;
            let operation = engine.RecognizeAsync(&bitmap)?;
            let started = Instant::now();
            while operation.Status()?.0 == 0 {
                if started.elapsed() >= TIMEOUT {
                    let _ = operation.Cancel();
                    return Err(windows::core::Error::new(
                        windows::core::HRESULT(0x800705B4u32 as i32),
                        "Windows OCR 응답 시간이 초과됐습니다.",
                    ));
                }
                thread::sleep(Duration::from_millis(20));
            }
            let result = operation.GetResults()?;
            let mut lines = Vec::new();
            let mut count = 0;
            'lines: for line in result.Lines()? {
                let mut words = Vec::new();
                for observed in line.Words()? {
                    if count == MAX_WORDS {
                        break 'lines;
                    }
                    count += 1;
                    let bounds = observed.BoundingRect()?;
                    if let Some(word) = crop.word(
                        &observed.Text()?.to_string(),
                        [
                            bounds.X as f64,
                            bounds.Y as f64,
                            bounds.Width as f64,
                            bounds.Height as f64,
                        ],
                    ) {
                        words.push(word);
                    }
                }
                if let Some(observed) = line_box(&words, Some(&line.Text()?.to_string())) {
                    lines.push(observed);
                }
            }
            Ok(lines)
        })();
        drop(apartment);
        result.map_err(|error| {
            format!("Windows OCR이 실패했습니다. 언어팩과 읽을 영역을 확인하세요: {error}")
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_reads_the_requested_pixels_in_bgra_rows_and_returns_full_frame_coordinates() {
        let mut bytes = vec![0; 100 * 60 * 4];
        for y in 0..60 {
            for x in 0..100 {
                let at = (y * 100 + x) * 4;
                bytes[at..at + 4].copy_from_slice(&[x as u8, y as u8, 200, 255]);
            }
        }
        let frame = Frame::new(&bytes, 100, 60, 400, 4, true).unwrap();
        let crop = Crop::new(&frame, Rect(0.25, 0.5, 0.75, 1.)).unwrap();
        let image = ppm(&frame, &crop);
        let header = b"P6\n100 60\n255\n";
        assert!(image.starts_with(header));
        assert_eq!(image.len(), header.len() + 100 * 60 * 3);
        assert_eq!(
            &image[header.len()..header.len() + 6],
            &[200, 30, 25, 200, 30, 25]
        );
        let text = "level\tpage_num\tblock_num\tpar_num\tline_num\tword_num\tleft\ttop\twidth\theight\tconf\ttext\n5\t1\t1\t1\t1\t1\t20\t10\t40\t20\t85.5\t친구 요청";
        let words = parse_tsv(text, &crop);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "친구 요청");
        let r = words[0].rect;
        for (actual, expected) in
            [r.0, r.1, r.2, r.3]
                .into_iter()
                .zip([0.35, 35. / 60., 0.55, 45. / 60.])
        {
            assert!((actual - expected).abs() < 1e-9);
        }
    }

    #[test]
    fn invalid_regions_and_untrusted_ocr_boxes_never_become_screen_coordinates() {
        let bytes = vec![0; 100 * 100 * 4];
        let frame = Frame::new(&bytes, 100, 100, 400, 4, false).unwrap();
        for rect in [
            Rect(-0.1, 0., 1., 1.),
            Rect(0., 0., f64::NAN, 1.),
            Rect(0.8, 0.2, 0.1, 0.9),
            Rect(0., 0., 0., 1.),
        ] {
            assert!(Crop::new(&frame, rect).is_err());
        }
        let crop = Crop::new(&frame, Rect(0., 0., 1., 1.)).unwrap();
        let tsv="5\t1\t1\t1\t1\t1\t0\t0\t20\t20\t-1\tnegative\n5\t1\t1\t1\t1\t2\t0\t0\t99999\t20\t90\toutside\n5\t1\t1\t1\t1\t3\t0\t0\t20\t20\tNaN\tinvalid\n5\t1\t1\t1\t1\t4\t2\t3\t20\t10\t90\tFish";
        let words = parse_tsv(tsv, &crop);
        assert_eq!(words.len(), 1);
        assert_eq!(words[0].text, "Fish");
    }

    #[test]
    fn tsv_groups_actual_line_ids_into_full_phrases_without_merging_other_blocks() {
        let bytes = vec![0; 400 * 200 * 4];
        let frame = Frame::new(&bytes, 400, 200, 1600, 4, false).unwrap();
        let crop = Crop::new(&frame, Rect(0., 0., 1., 1.)).unwrap();
        let tsv = "5\t1\t1\t1\t1\t1\t10\t20\t20\t12\t95\tSet\n5\t1\t1\t1\t1\t2\t40\t20\t15\t12\t95\tto\n5\t1\t2\t1\t1\t1\t200\t20\t20\t12\t95\tOpen\n5\t1\t1\t1\t1\t3\t65\t20\t30\t12\t95\tMax\n5\t1\t2\t1\t1\t2\t230\t20\t50\t12\t95\tRecipe\n5\t2\t1\t1\t1\t1\t10\t20\t40\t12\t95\tOther";
        let lines = parse_tsv(tsv, &crop);
        assert_eq!(
            lines
                .iter()
                .map(|word| word.text.as_str())
                .collect::<Vec<_>>(),
            ["Set to Max", "Open Recipe", "Other"]
        );
        assert!((lines[0].rect.0 - 10. / 400.).abs() < 1e-9);
        assert!((lines[0].rect.2 - 95. / 400.).abs() < 1e-9);
        assert!((lines[1].rect.0 - 200. / 400.).abs() < 1e-9);
        assert!((lines[1].rect.2 - 280. / 400.).abs() < 1e-9);
    }

    #[test]
    fn ordinary_720p_regions_are_not_expanded_to_the_entire_screen() {
        let bytes = vec![0; 1280 * 720 * 4];
        let frame = Frame::new(&bytes, 1280, 720, 5120, 4, false).unwrap();
        let crop = Crop::new(&frame, Rect(0.25, 0.25, 0.75, 0.75)).unwrap();
        assert_eq!(
            (crop.left, crop.top, crop.width, crop.height),
            (320, 180, 640, 360)
        );
        assert_eq!((crop.output_width, crop.output_height), (640, 360));
        assert_eq!(
            ppm(&frame, &crop).len(),
            b"P6\n640 360\n255\n".len() + 640 * 360 * 3
        );
    }
}
