use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Point(pub f64, pub f64);

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Rect(pub f64, pub f64, pub f64, pub f64);

/// A captured rectangle in the original viewport's pixel coordinates.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Region {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

impl Region {
    pub fn full(width: usize, height: usize) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    pub fn valid(self, geometry: (usize, usize)) -> bool {
        self.width > 0
            && self.height > 0
            && self
                .x
                .checked_add(self.width)
                .is_some_and(|right| right <= geometry.0)
            && self
                .y
                .checked_add(self.height)
                .is_some_and(|bottom| bottom <= geometry.1)
    }
}

impl Rect {
    pub fn at(self, x: f64, y: f64) -> Point {
        Point(
            self.0 + (self.2 - self.0) * x,
            self.1 + (self.3 - self.1) * y,
        )
    }

    pub fn inset(self, x0: f64, y0: f64, x1: f64, y1: f64) -> Self {
        let Point(left, top) = self.at(x0, y0);
        let Point(right, bottom) = self.at(x1, y1);
        Self(left, top, right, bottom)
    }

    pub fn region(self, (width, height): (usize, usize)) -> Option<Region> {
        if width == 0
            || height == 0
            || ![self.0, self.1, self.2, self.3]
                .iter()
                .all(|v| v.is_finite())
        {
            return None;
        }
        let x0 = (self.0.clamp(0.0, 1.0) * width as f64).floor() as usize;
        let y0 = (self.1.clamp(0.0, 1.0) * height as f64).floor() as usize;
        let x1 = (self.2.clamp(0.0, 1.0) * width as f64).ceil() as usize;
        let y1 = (self.3.clamp(0.0, 1.0) * height as f64).ceil() as usize;
        let region = Region {
            x: x0,
            y: y0,
            width: x1.checked_sub(x0)?,
            height: y1.checked_sub(y0)?,
        };
        region.valid((width, height)).then_some(region)
    }
}

pub struct Frame<'a> {
    bytes: &'a [u8],
    pub width: usize,
    pub height: usize,
    region: Region,
    stride: usize,
    pixel_stride: usize,
    bgra: bool,
}

impl<'a> Frame<'a> {
    pub fn new(
        bytes: &'a [u8],
        width: usize,
        height: usize,
        stride: usize,
        pixel_stride: usize,
        bgra: bool,
    ) -> Option<Self> {
        Self::region(
            bytes,
            width,
            height,
            stride,
            pixel_stride,
            bgra,
            Region::full(width, height),
        )
    }

    pub fn region(
        bytes: &'a [u8],
        width: usize,
        height: usize,
        stride: usize,
        pixel_stride: usize,
        bgra: bool,
        region: Region,
    ) -> Option<Self> {
        if !region.valid((width, height)) {
            return None;
        }
        let row = region
            .width
            .checked_sub(1)?
            .checked_mul(pixel_stride)?
            .checked_add(3)?;
        let len = region
            .height
            .checked_sub(1)?
            .checked_mul(stride)?
            .checked_add(row)?;
        (pixel_stride >= 3 && stride >= row && bytes.len() >= len).then_some(Self {
            bytes,
            width,
            height,
            region,
            stride,
            pixel_stride,
            bgra,
        })
    }

    fn pixel(&self, x: usize, y: usize) -> Option<[u8; 3]> {
        let x = x.checked_sub(self.region.x)?;
        let y = y.checked_sub(self.region.y)?;
        if x >= self.region.width || y >= self.region.height {
            return None;
        }
        let p = y
            .checked_mul(self.stride)?
            .checked_add(x.checked_mul(self.pixel_stride)?)?;
        let [r, g, b]: [u8; 3] = self.bytes.get(p..p.checked_add(3)?)?.try_into().ok()?;
        Some(if self.bgra { [b, g, r] } else { [r, g, b] })
    }

    pub fn is_full(&self) -> bool {
        self.region == Region::full(self.width, self.height)
    }

    pub fn coverage(&self) -> Region {
        self.region
    }

    fn contains_bounds(&self, (left, top, right, bottom): (usize, usize, usize, usize)) -> bool {
        left < right
            && top < bottom
            && left >= self.region.x
            && top >= self.region.y
            && right <= self.region.x + self.region.width
            && bottom <= self.region.y + self.region.height
    }

    pub fn contains(&self, rect: Rect) -> bool {
        self.contains_bounds(self.bounds(rect))
    }

    pub fn position(&self, point: Point) -> (usize, usize) {
        let x = ((point.0 * self.width as f64) as usize).min(self.width - 1);
        let y = ((point.1 * self.height as f64) as usize).min(self.height - 1);
        (x, y)
    }

    fn bounds(&self, rect: Rect) -> (usize, usize, usize, usize) {
        if ![rect.0, rect.1, rect.2, rect.3]
            .iter()
            .all(|value| value.is_finite())
            || rect.2 <= rect.0
            || rect.3 <= rect.1
        {
            return (0, 0, 0, 0);
        }
        let x = |v: f64| ((v * self.width as f64) as usize).min(self.width);
        let y = |v: f64| ((v * self.height as f64) as usize).min(self.height);
        (x(rect.0), y(rect.1), x(rect.2), y(rect.3))
    }

    fn centered(point: Point, width: f64, height: f64) -> Rect {
        Rect(
            point.0 - width / 2.0,
            point.1 - height / 2.0,
            point.0 + width / 2.0,
            point.1 + height / 2.0,
        )
    }

    pub fn sale_color(&self, point: Point, green: bool) -> f64 {
        let width = (self.height as f64 / self.width as f64) * 0.08;
        let (left, top, right, bottom) = self.bounds(Self::centered(point, width, 0.04));
        if !self.contains_bounds((left, top, right, bottom)) {
            return f64::NAN;
        }
        let mut count = 0;
        for y in top..bottom {
            for x in left..right {
                let Some(pixel) = self.pixel(x, y) else {
                    return f64::NAN;
                };
                let [r, g, b] = pixel.map(i16::from);
                let matches = if green {
                    g > 120 && g > r + 30 && g > b + 30
                } else {
                    b > 100 && b > r + 35 && b > g + 15
                };
                count += usize::from(matches);
            }
        }
        count as f64 / ((right - left) * (bottom - top)).max(1) as f64
    }

    pub fn button(&self, point: Point) -> Option<Button> {
        let (left, top, right, bottom) = self.bounds(Self::centered(point, 0.045, 0.035));
        if !self.contains_bounds((left, top, right, bottom)) {
            return None;
        }
        let total = (right - left) * (bottom - top);
        let (mut dark, mut blue, mut red) = (0, 0, 0);
        for y in top..bottom {
            for x in left..right {
                let [r, g, b] = self.pixel(x, y)?.map(i16::from);
                dark += usize::from(r.max(g).max(b) < 70);
                blue += usize::from(b > 140 && b > r + 45 && b > g + 20);
                red += usize::from(r > 150 && r > g + 60 && r > b + 40);
            }
        }
        if total == 0 || dark * 100 < total * 45 || blue.max(red) * 100 < total {
            return None;
        }
        Some(if blue >= red {
            Button::Fish
        } else {
            Button::Exit
        })
    }

    pub fn diamond(&self, point: Point, radius: usize) -> Option<bool> {
        let (cx, cy) = self.position(point);
        let bounds = (
            cx.saturating_sub(radius),
            cy.saturating_sub(radius),
            cx.saturating_add(radius).min(self.width - 1) + 1,
            cy.saturating_add(radius).min(self.height - 1) + 1,
        );
        if !self.contains_bounds(bounds) {
            return None;
        }
        let mut count = 0;
        for y in bounds.1..bounds.3 {
            for x in bounds.0..bounds.2 {
                count += usize::from(min(self.pixel(x, y)?) > 200);
                if count >= 6 {
                    return Some(true);
                }
            }
        }
        Some(false)
    }

    pub fn title(&self, point: Point) -> Option<Catch> {
        let (x0, y0, x1, y1) = self.bounds(Self::centered(point, 0.12, 0.05));
        if !self.contains_bounds((x0, y0, x1, y1)) {
            return None;
        }
        let (w, h) = (x1 - x0, y1 - y0);
        for (kind, count) in [(Catch::Fail, 12), (Catch::Success, 12), (Catch::Junk, 20)] {
            let mut pixels = 0;
            let mut cols = 0;
            let mut rows = 0u64;
            for x in x0..x1 {
                let mut found = false;
                // At most 64 representative rows, including the title's thin strokes.
                let stride = h.div_ceil(64).max(1);
                for y in (y0..y1).step_by(stride) {
                    let p = self.pixel(x, y)?;
                    let is_fail = distance(p, [255, 65, 65]) <= 8;
                    let matches = match kind {
                        Catch::Fail => is_fail,
                        Catch::Junk => distance(p, [186; 3]) <= 8,
                        Catch::Success => {
                            !is_fail && (min(p) >= 232 || (max(p) >= 220 && max(p) - min(p) >= 50))
                        }
                    };
                    if matches {
                        pixels += stride;
                        rows |= 1 << ((y - y0) / stride);
                        found = true;
                    }
                }
                cols += usize::from(found);
            }
            let density = if kind == Catch::Junk { 0.02 } else { 0.006 };
            if pixels >= count
                && pixels as f64 >= (w * h) as f64 * density
                && cols * 10 >= w * 3
                && rows.count_ones() as usize * h.div_ceil(64).max(1) >= 4.max(h * 15 / 100)
            {
                return Some(kind);
            }
        }
        None
    }

    pub fn result_close(&self, window: Rect) -> Option<Point> {
        let expected = window.at(0.9222, 0.0579);
        let (cx, cy) = self.position(expected);
        let (left, top, right, bottom) = self.bounds(window.inset(0.6, -0.03, 1.05, 0.25));
        if !self.contains_bounds((left, top, right, bottom)) {
            return None;
        }
        let (w, h) = (right - left, bottom - top);
        let lower = (self.height * 4 / 1000).max(3);
        let upper = (self.height * 35 / 1000).max(lower);
        let mut labels = vec![0u32; w * h];
        let mut queue = Vec::new();
        let mut id = 0;
        let mut best = None;
        for y in 0..h {
            for x in 0..w {
                if labels[y * w + x] != 0 || min(self.pixel(left + x, top + y)?) <= 150 {
                    continue;
                }
                id += 1;
                queue.clear();
                queue.push((x, y));
                labels[y * w + x] = id;
                let (mut x0, mut y0, mut x1, mut y1) = (x, y, x, y);
                let mut index = 0;
                while index < queue.len() {
                    let (x, y) = queue[index];
                    index += 1;
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x);
                    y1 = y1.max(y);
                    for yy in y.saturating_sub(1)..=(y + 1).min(h - 1) {
                        for xx in x.saturating_sub(1)..=(x + 1).min(w - 1) {
                            let i = yy * w + xx;
                            if labels[i] == 0 && min(self.pixel(left + xx, top + yy)?) > 150 {
                                labels[i] = id;
                                queue.push((xx, yy));
                            }
                        }
                    }
                }
                let (bw, bh) = (x1 - x0 + 1, y1 - y0 + 1);
                if !(lower..=upper).contains(&bw)
                    || !(lower..=upper).contains(&bh)
                    || !(0.7..=1.4).contains(&(bw as f64 / bh as f64))
                    || !(0.15..=0.65).contains(&(queue.len() as f64 / (bw * bh) as f64))
                {
                    continue;
                }
                let diagonal = |reverse| {
                    (1..9)
                        .filter(|&i| {
                            let y = y0 + i * (bh - 1) / 9;
                            let x = x0 + (if reverse { 9 - i } else { i }) * (bw - 1) / 9;
                            (y.saturating_sub(1)..=(y + 1).min(y1)).any(|yy| {
                                (x.saturating_sub(1)..=(x + 1).min(x1))
                                    .any(|xx| labels[yy * w + xx] == id)
                            })
                        })
                        .count()
                        >= 7
                };
                if diagonal(false) && diagonal(true) {
                    let (x, y) = (left + (x0 + x1) / 2, top + (y0 + y1) / 2);
                    let distance = x.abs_diff(cx).pow(2) + y.abs_diff(cy).pow(2);
                    if best.is_none_or(|(nearest, _, _)| distance < nearest) {
                        best = Some((distance, x, y));
                    }
                }
            }
        }
        best.map(|(_, x, y)| {
            Point(
                (x as f64 + 0.5) / self.width as f64,
                (y as f64 + 0.5) / self.height as f64,
            )
        })
    }
}

#[derive(PartialEq, Eq)]
pub enum Button {
    Fish,
    Exit,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Catch {
    Success,
    Junk,
    Fail,
}

fn max(p: [u8; 3]) -> u8 {
    p[0].max(p[1]).max(p[2])
}
fn min(p: [u8; 3]) -> u8 {
    p[0].min(p[1]).min(p[2])
}
fn distance(a: [u8; 3], b: [u8; 3]) -> u8 {
    a[0].abs_diff(b[0])
        .max(a[1].abs_diff(b[1]))
        .max(a[2].abs_diff(b[2]))
}

fn colored(p: [u8; 3]) -> bool {
    max(p) > 90 && max(p) - min(p) > 50
}
fn fill(p: [u8; 3]) -> bool {
    let [r, g, b] = p.map(i16::from);
    r < 100 && g >= 80 && b >= 140 && (-15..=85).contains(&(b - g))
}

#[derive(Default)]
pub struct Bar {
    band: Vec<[u8; 3]>,
    sample: Vec<u8>,
    counts: Vec<usize>,
    zones: Vec<(f64, f64)>,
    fill: Option<[u8; 3]>,
}

impl Bar {
    pub fn reset(&mut self) {
        self.fill = None;
    }

    pub fn read(&mut self, frame: &Frame<'_>, area: Rect) -> bool {
        let (x0, y0, x1, y1) = frame.bounds(area.inset(0.0, 0.35, 1.0, 0.65));
        let w = x1.saturating_sub(x0);
        if w < 8 || !frame.contains_bounds((x0, y0, x1, y1)) {
            self.band.clear();
            return false;
        }
        self.band.resize(w, [0; 3]);
        self.sample.resize(y1 - y0, 0);
        let mid = self.sample.len() / 2;
        for (x, color) in self.band.iter_mut().enumerate() {
            for (channel, value) in color.iter_mut().enumerate() {
                for (y, sample) in self.sample.iter_mut().enumerate() {
                    let Some(pixel) = frame.pixel(x0 + x, y0 + y) else {
                        return false;
                    };
                    *sample = pixel[channel];
                }
                *value = *self.sample.select_nth_unstable(mid).1;
            }
        }
        let dark_or_fill = |p| max(p) < 45 || fill(p);
        let head = (w * 4 / 100).max(3);
        let colored_count = self.band.iter().filter(|&&p| colored(p)).count();
        let dark_count = self.band.iter().filter(|&&p| max(p) < 45).count();
        colored_count >= 4.max(w.div_ceil(34))
            && (self.band[..head]
                .iter()
                .filter(|&&p| dark_or_fill(p))
                .count()
                * 2
                >= head
                || dark_count * 100 >= w * 8)
    }

    pub fn track(&mut self, frame: &Frame<'_>, area: Rect) -> (Option<f64>, &[(f64, f64)], f64) {
        let w = self.band.len();
        self.zones.clear();
        if w == 0 {
            return (None, &self.zones, 0.0);
        }
        if self.fill.is_none() {
            let head = &self.band[1..(w * 15 / 100).max(3)];
            let reference = head[head.len() / 2];
            if fill(reference)
                && head
                    .iter()
                    .filter(|&&p| fill(p) && distance(p, reference) <= 12)
                    .count()
                    * 10
                    >= head.len() * 9
            {
                self.fill = Some(reference);
            }
        }
        let mut segment = None;
        let mut fill_end = None;
        for (x, &p) in self.band.iter().enumerate() {
            let is_fill = self.fill.map_or_else(
                || colored(p) && fill(p),
                |reference| max(p) > 75 && distance(p, reference) <= 22,
            );
            let text = if self.fill.is_some() {
                self.band[x.saturating_sub(2)..=(x + 2).min(w - 1)]
                    .iter()
                    .any(|&p| min(p) > 185)
            } else {
                max(p) > 90 && !colored(p)
            };
            let zone = !is_fill
                && !text
                && if self.fill.is_some() {
                    max(p) > 75
                } else {
                    colored(p)
                };
            if is_fill {
                fill_end = Some(x);
            }
            if zone {
                let (start, _, _) = segment.unwrap_or((x, x, 0));
                segment = Some((start, x, 0));
            } else if let Some((start, end, gap)) = segment.filter(|_| !text) {
                if gap == 4 {
                    if end - start >= 3 {
                        self.zones.push((start as f64, end as f64));
                    }
                    segment = None;
                } else {
                    segment = Some((start, end, gap + 1));
                }
            }
        }
        if let Some((start, end, _)) = segment.filter(|(start, end, _)| end - start >= 3) {
            self.zones.push((start as f64, end as f64));
        }

        let (left, top, _, bottom) = frame.bounds(area.inset(0.0, -1.7, 1.0, 0.0));
        if !frame.contains_bounds((left, top, left + w, bottom)) {
            self.zones.clear();
            return (None, &self.zones, w as f64);
        }
        let rows = bottom - top;
        self.counts.resize(w, 0);
        self.counts.fill(0);
        for y in top..bottom.saturating_sub(1) {
            for x in 0..w {
                self.counts[x] += usize::from(
                    frame
                        .pixel(left + x, y)
                        .is_some_and(|pixel| min(pixel) > 185),
                );
            }
        }
        let radius = (w * 5 / 100).max(5) / 2;
        let count = |x: usize| {
            let n = self.counts[x];
            let neighbor =
                |i: Option<usize>| i.and_then(|i| self.counts.get(i)).copied().unwrap_or(0);
            let side = neighbor(x.checked_sub(2)).max(neighbor(x.checked_add(2)));
            if n * 10 >= rows * 8 && side * 10 < n * 3 {
                0
            } else {
                n
            }
        };
        let mut sum: usize = (0..=radius).map(count).sum();
        let mut best = (sum, 0);
        for x in 1..w {
            if x > radius {
                sum -= count(x - radius - 1);
            }
            if x + radius < w {
                sum += count(x + radius);
            }
            if sum > best.0 {
                best = (sum, x);
            }
        }
        let lo = best.1.saturating_sub(radius * 2);
        let hi = (best.1 + radius * 2 + 1).min(w);
        let (mass, weighted) = (lo..hi).fold((0, 0), |(mass, weighted), x| {
            let c = count(x);
            (mass + c, weighted + x * c)
        });
        let marker = (mass >= 6).then(|| weighted as f64 / mass as f64);
        let end = fill_end.unwrap_or(0);
        let end_on_zone = self
            .zones
            .iter()
            .any(|&(a, b)| a - 3.0 <= end as f64 && end as f64 <= b + 2.0);
        let end_has_gap = self.band[end + 1..(end + 1 + (w / 40).max(8)).min(w)]
            .iter()
            .all(|&p| !colored(p) && max(p) <= 90);
        let fill_marker = (fill_end.is_some() && !end_on_zone && end_has_gap).then_some(end as f64);
        let marker = match (marker, fill_marker) {
            (Some(m), Some(f)) if (m - f).abs() > (w as f64 * 0.04).max(12.0) => Some(f),
            (m, f) => m.or(f),
        };
        (marker, &self.zones, w as f64)
    }
}

#[derive(Default)]
pub struct WindowFinder {
    mask: Vec<u8>,
    top_left: Vec<(usize, usize)>,
    bottom_right: Vec<(usize, usize)>,
}

impl WindowFinder {
    pub fn find(&mut self, frame: &Frame<'_>, accepts: impl Fn(Rect, f64) -> bool) -> Option<Rect> {
        // One flag byte per sampled pixel, reused for threshold and the four arm directions.
        const WHITE: u8 = 1;
        const LEFT: u8 = 2;
        const RIGHT: u8 = 4;
        const UP: u8 = 8;
        const DOWN: u8 = 16;
        let step = frame.width.max(frame.height).div_ceil(1280);
        let region = frame.region;
        let (w, h) = (region.width.div_ceil(step), region.height.div_ceil(step));
        if w < 40 || h < 40 {
            return None;
        }
        let arm = (frame.width.div_ceil(step) / 250).max(4);
        self.mask.resize(w * h, 0);
        let has = |flags: u8, required: u8| flags & required == required;

        for threshold in [215, 238] {
            for y in 0..h {
                for x in 0..w {
                    let (x0, y0) = (region.x + x * step, region.y + y * step);
                    let white = (y0..(y0 + step).min(region.y + region.height)).any(|yy| {
                        (x0..(x0 + step).min(region.x + region.width)).any(|xx| {
                            frame
                                .pixel(xx, yy)
                                .is_some_and(|p| min(p) > threshold && max(p) - min(p) < 35)
                        })
                    });
                    self.mask[y * w + x] = if white { WHITE } else { 0 };
                }
            }
            for y in 0..h {
                let row = &mut self.mask[y * w..(y + 1) * w];
                let mut len = 0;
                for cell in row.iter_mut() {
                    len = if has(*cell, WHITE) { len + 1 } else { 0 };
                    if len >= arm {
                        *cell |= LEFT;
                    }
                }
                len = 0;
                for cell in row.iter_mut().rev() {
                    len = if has(*cell, WHITE) { len + 1 } else { 0 };
                    if len >= arm {
                        *cell |= RIGHT;
                    }
                }
            }
            for x in 0..w {
                let mut len = 0;
                for cell in self.mask[x..].iter_mut().step_by(w) {
                    len = if has(*cell, WHITE) { len + 1 } else { 0 };
                    if len >= arm {
                        *cell |= UP;
                    }
                }
                len = 0;
                for y in (0..h).rev() {
                    let cell = &mut self.mask[y * w + x];
                    len = if has(*cell, WHITE) { len + 1 } else { 0 };
                    if len >= arm {
                        *cell |= DOWN;
                    }
                }
            }

            self.top_left.clear();
            self.bottom_right.clear();
            let mask = &self.mask;
            let white = |x: usize, y: usize| has(mask[y * w + x], WHITE);
            for y in 0..h {
                for x in 0..w {
                    let flags = mask[y * w + x];
                    if has(flags, RIGHT | DOWN)
                        && (x == 0 || !white(x - 1, y))
                        && (y == 0 || !white(x, y - 1))
                    {
                        self.top_left.push((x, y));
                    }
                    if has(flags, LEFT | UP)
                        && (x + 1 == w || !white(x + 1, y))
                        && (y + 1 == h || !white(x, y + 1))
                    {
                        self.bottom_right.push((x, y));
                    }
                }
            }
            let near = |x: usize, y: usize, flags| {
                (y.saturating_sub(3)..=(y + 3).min(h - 1)).any(|yy| {
                    (x.saturating_sub(3)..=(x + 3).min(w - 1))
                        .any(|xx| has(mask[yy * w + xx], flags))
                })
            };
            let mut best = None;
            let mut best_area = 0;
            for &(x0, y0) in &self.top_left {
                for &(x1, y1) in &self.bottom_right {
                    let (dx, dy) = (x1.saturating_sub(x0), y1.saturating_sub(y0));
                    let area = dx * dy;
                    if dx * step < frame.width / 10
                        || dx * step > frame.width * 92 / 100
                        || dy < 3 * arm
                        || area <= best_area
                    {
                        continue;
                    }
                    let x = |n| (region.x + n * step) as f64 / frame.width as f64;
                    let y = |n| (region.y + n * step) as f64 / frame.height as f64;
                    let rect = Rect(x(x0), y(y0), x(x1), y(y1));
                    if near(x1, y0, LEFT | DOWN)
                        && near(x0, y1, RIGHT | UP)
                        && accepts(rect, dx as f64 / dy as f64)
                    {
                        best = Some(rect);
                        best_area = area;
                    }
                }
            }
            if best.is_some() {
                return best;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crop(bytes: &[u8], stride: usize, region: Region) -> (Vec<u8>, usize) {
        let cropped_stride = region.width * 4 + 12;
        let mut cropped = vec![0; cropped_stride * region.height];
        for y in 0..region.height {
            let start = (region.y + y) * stride + region.x * 4;
            cropped[y * cropped_stride..y * cropped_stride + region.width * 4]
                .copy_from_slice(&bytes[start..start + region.width * 4]);
        }
        (cropped, cropped_stride)
    }

    #[test]
    fn crop_preserves_viewport_points_button_diamond_and_reel_evidence() {
        for (width, height, bgra) in [
            (320, 200, false),
            (640, 400, true),
            (400, 400, false),
            (1920, 1080, true),
        ] {
            let stride = width * 4 + 16;
            let mut bytes = vec![0; stride * height];
            let mut put = |x: usize, y: usize, mut pixel: [u8; 3]| {
                if bgra {
                    pixel.reverse();
                }
                let p = y * stride + x * 4;
                bytes[p..p + 3].copy_from_slice(&pixel);
            };
            for y in height * 80 / 100..height * 81 / 100 {
                for x in width * 37 / 100..width * 38 / 100 {
                    put(x, y, [20, 75, 205]);
                }
            }
            for y in height * 40 / 100..height * 45 / 100 {
                for x in width * 42 / 100..width * 50 / 100 {
                    put(x, y, [55, 180, 40]);
                }
            }
            for y in height * 25 / 100..height * 33 / 100 {
                for x in width * 34 / 100..width * 35 / 100 {
                    put(x, y, [240; 3]);
                }
            }
            for y in height * 80 / 100..height * 80 / 100 + 3 {
                for x in width * 70 / 100..width * 70 / 100 + 3 {
                    put(x, y, [245; 3]);
                }
            }
            let full = Frame::new(&bytes, width, height, stride, 4, bgra).unwrap();
            let region = Region {
                x: width / 5,
                y: height / 20,
                width: width * 3 / 5,
                height: height * 9 / 10,
            };
            let (cropped, cropped_stride) = crop(&bytes, stride, region);
            let cropped =
                Frame::region(&cropped, width, height, cropped_stride, 4, bgra, region).unwrap();
            let button = Point(0.375, 0.805);
            assert!(full.button(button) == Some(Button::Fish));
            assert!(cropped.button(button) == full.button(button));
            assert_eq!(cropped.position(button), full.position(button));
            assert_eq!(cropped.diamond(Point(0.7, 0.8), 4), Some(true));
            assert_eq!(
                cropped.diamond(Point(0.7, 0.8), 4),
                full.diamond(Point(0.7, 0.8), 4)
            );
            let area = Rect(0.25, 0.35, 0.75, 0.5);
            let mut a = Bar::default();
            let mut b = Bar::default();
            assert!(a.read(&full, area));
            assert!(b.read(&cropped, area));
            let a = a.track(&full, area);
            let b = b.track(&cropped, area);
            assert_eq!(a, b);
            assert!(a.0.is_some() && !a.1.is_empty());
        }
    }

    #[test]
    fn missing_pixels_are_unknown_and_invalid_crops_are_rejected() {
        let region = Region {
            x: 100,
            y: 60,
            width: 20,
            height: 20,
        };
        let bytes = vec![0; 20 * 20 * 4];
        let frame = Frame::region(&bytes, 320, 200, 80, 4, false, region).unwrap();
        let missing = Point(0.1, 0.1);
        assert!(frame.button(missing).is_none());
        assert_eq!(frame.diamond(missing, 3), None);
        assert!(frame.title(missing).is_none());
        assert!(frame.sale_color(missing, false).is_nan());
        assert!(frame.result_close(Rect(0.2, 0.1, 0.8, 0.9)).is_none());
        let mut bar = Bar::default();
        assert!(!bar.read(&frame, Rect(0.25, 0.35, 0.75, 0.5)));
        assert!(bar.track(&frame, Rect(0.25, 0.35, 0.75, 0.5)).0.is_none());
        assert!(Frame::region(&bytes, 320, 200, 78, 4, false, region).is_none());
        assert!(Frame::region(&bytes[..bytes.len() - 2], 320, 200, 80, 4, false, region).is_none());
        assert!(Frame::region(&bytes, 110, 200, 80, 4, false, region).is_none());
        assert!(!Region {
            x: usize::MAX,
            y: 0,
            width: 2,
            height: 1
        }
        .valid((320, 200)));
        assert_eq!(
            Rect(-0.1, -0.1, 0.1, 0.1).region((320, 200)),
            Some(Region {
                x: 0,
                y: 0,
                width: 32,
                height: 20
            })
        );
        assert!(Rect(f64::NAN, 0.0, 1.0, 1.0).region((320, 200)).is_none());
    }

    #[test]
    fn no_marker_pixels_does_not_mean_a_marker_at_zero() {
        let (width, height) = (320, 200);
        let mut bytes = vec![0; width * height * 4];
        for y in 80..90 {
            for x in 130..160 {
                let p = (y * width + x) * 4;
                bytes[p..p + 3].copy_from_slice(&[55, 180, 40]);
            }
        }
        let frame = Frame::new(&bytes, width, height, width * 4, 4, false).unwrap();
        let mut bar = Bar::default();
        let area = Rect(0.25, 0.35, 0.75, 0.5);
        assert!(bar.read(&frame, area));
        let (marker, zones, _) = bar.track(&frame, area);
        assert!(marker.is_none() && !zones.is_empty());
    }
}
