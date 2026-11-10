use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Point(pub f64, pub f64);

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Rect(pub f64, pub f64, pub f64, pub f64);

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
}

pub struct Frame<'a> {
    bytes: &'a [u8],
    pub width: usize,
    pub height: usize,
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
        let row = width
            .checked_sub(1)?
            .checked_mul(pixel_stride)?
            .checked_add(3)?;
        let len = height
            .checked_sub(1)?
            .checked_mul(stride)?
            .checked_add(row)?;
        (pixel_stride >= 3 && stride >= row && bytes.len() >= len).then_some(Self {
            bytes,
            width,
            height,
            stride,
            pixel_stride,
            bgra,
        })
    }

    fn pixel(&self, x: usize, y: usize) -> [u8; 3] {
        let p = y * self.stride + x * self.pixel_stride;
        let [r, g, b] = [self.bytes[p], self.bytes[p + 1], self.bytes[p + 2]];
        if self.bgra {
            [b, g, r]
        } else {
            [r, g, b]
        }
    }

    pub fn position(&self, point: Point) -> (usize, usize) {
        let x = ((point.0 * self.width as f64) as usize).min(self.width - 1);
        let y = ((point.1 * self.height as f64) as usize).min(self.height - 1);
        (x, y)
    }

    fn bounds(&self, rect: Rect) -> (usize, usize, usize, usize) {
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
        let mut count = 0;
        for y in top..bottom {
            for x in left..right {
                let [r, g, b] = self.pixel(x, y).map(i16::from);
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
        let total = (right - left) * (bottom - top);
        let (mut dark, mut blue, mut red) = (0, 0, 0);
        for y in top..bottom {
            for x in left..right {
                let [r, g, b] = self.pixel(x, y).map(i16::from);
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

    pub fn diamond(&self, point: Point, radius: usize) -> bool {
        let (cx, cy) = self.position(point);
        let mut count = 0;
        for y in cy.saturating_sub(radius)..=(cy + radius).min(self.height - 1) {
            for x in cx.saturating_sub(radius)..=(cx + radius).min(self.width - 1) {
                count += usize::from(min(self.pixel(x, y)) > 200);
                if count >= 6 {
                    return true;
                }
            }
        }
        false
    }

    pub fn title(&self, point: Point) -> Option<Catch> {
        let (x0, y0, x1, y1) = self.bounds(Self::centered(point, 0.12, 0.05));
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
                    let p = self.pixel(x, y);
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

    pub fn result_close(&self, window: Rect) -> Point {
        let expected = window.at(0.9222, 0.0579);
        let (cx, cy) = self.position(expected);
        let (left, top, right, bottom) = self.bounds(window.inset(0.6, -0.03, 1.05, 0.25));
        let (w, h) = (right - left, bottom - top);
        let lower = (self.height * 4 / 1000).max(3);
        let upper = (self.height * 35 / 1000).max(lower);
        let mut labels = vec![0u32; w * h];
        let mut queue = Vec::new();
        let mut id = 0;
        let mut best = None;
        for y in 0..h {
            for x in 0..w {
                if labels[y * w + x] != 0 || min(self.pixel(left + x, top + y)) <= 150 {
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
                            if labels[i] == 0 && min(self.pixel(left + xx, top + yy)) > 150 {
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
        best.map_or(expected, |(_, x, y)| {
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
        if w < 8 || y1 <= y0 {
            self.band.clear();
            return false;
        }
        self.band.resize(w, [0; 3]);
        self.sample.resize(y1 - y0, 0);
        let mid = self.sample.len() / 2;
        for (x, color) in self.band.iter_mut().enumerate() {
            for (channel, value) in color.iter_mut().enumerate() {
                for (y, sample) in self.sample.iter_mut().enumerate() {
                    *sample = frame.pixel(x0 + x, y0 + y)[channel];
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
        let rows = bottom - top;
        self.counts.resize(w, 0);
        self.counts.fill(0);
        for y in top..bottom.saturating_sub(1) {
            for x in 0..w {
                self.counts[x] += usize::from(min(frame.pixel(left + x, y)) > 185);
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
        let fill_marker = (!end_on_zone && end_has_gap).then_some(end as f64);
        let marker = match (marker, fill_marker) {
            (Some(m), Some(f)) if (m - f).abs() > (w as f64 * 0.04).max(12.0) => Some(f),
            (None, _) if fill_end.is_none() => Some(0.0),
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
        let (w, h) = (frame.width.div_ceil(step), frame.height.div_ceil(step));
        if w < 40 || h < 40 {
            return None;
        }
        let arm = (w / 250).max(4);
        self.mask.resize(w * h, 0);
        let has = |flags: u8, required: u8| flags & required == required;

        for threshold in [215, 238] {
            for y in 0..h {
                for x in 0..w {
                    let (x0, y0) = (x * step, y * step);
                    let white = (y0..(y0 + step).min(frame.height)).any(|yy| {
                        (x0..(x0 + step).min(frame.width)).any(|xx| {
                            let p = frame.pixel(xx, yy);
                            min(p) > threshold && max(p) - min(p) < 35
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
                    if dx < w / 10 || dx > w * 92 / 100 || dy < 3 * arm || area <= best_area {
                        continue;
                    }
                    let x = |n| (n * step) as f64 / frame.width as f64;
                    let y = |n| (n * step) as f64 / frame.height as f64;
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
