//! Measured image translation for a fixed camera. This is not a world pose:
//! camera rotation, height, ground, heading and contact need separate evidence.
use crate::vision::{Frame, Region};

const SIDE: usize = 512;
const SEARCH: usize = 256;
const CELLS: usize = 8;
const MIN_MATCHES: usize = 8;

pub(crate) fn displacement_limit(geometry: (usize, usize)) -> usize {
    geometry.0.max(geometry.1).div_ceil(SIDE).max(1) * SEARCH
}

pub struct Snapshot {
    geometry: (usize, usize),
    included: Region,
    excluded: Vec<Region>,
    width: usize,
    height: usize,
    step: usize,
    descriptors: Vec<u64>,
    quality: Vec<u16>,
    features: Vec<(usize, usize)>,
    gray: Vec<u8>,
    valid: Vec<bool>,
}

#[derive(Clone, Copy, Debug)]
pub struct Translation {
    /// Image texture moves from (x,y) to (x+dx,y+dy), in viewport pixels.
    pub dx: f64,
    pub dy: f64,
    /// Registration scatter plus sampling uncertainty, not a world error bound.
    pub error_pixels: f64,
    pub matches: usize,
    pub features: usize,
}

impl Snapshot {
    /// Add a newly observed moving-body mask to an OLD snapshot without ever
    /// restoring unknown pixels. The caller captures the new frame with the
    /// same union, then rebuilds its next reference from that new frame.
    pub(crate) fn exclude_region(&mut self, region: Region) -> Result<(), String> {
        if !region.valid(self.geometry) || self.excluded.len() >= 32 {
            return Err("Scene moving-body mask is invalid.".into());
        }
        if !self.excluded.contains(&region) {
            self.excluded.push(region);
        }
        self.excluded.sort_by_key(|r| (r.x, r.y, r.width, r.height));
        let step = self.step;
        self.restrict_surface(|x, y| {
            let (left, top) = (x - step / 2, y - step / 2);
            !(left < region.x + region.width
                && left + step > region.x
                && top < region.y + region.height
                && top + step > region.y)
        });
        Ok(())
    }
    /// Retain only pixels witnessed on the requested local surface. The caller
    /// supplies a same-frame segmentation; this never creates missing pixels.
    pub(crate) fn restrict_surface(&mut self, accepts: impl Fn(usize, usize) -> bool) {
        for y in 0..self.height {
            for x in 0..self.width {
                if !accepts(x * self.step + self.step / 2, y * self.step + self.step / 2) {
                    self.valid[y * self.width + x] = false;
                    self.quality[y * self.width + x] = 0;
                }
            }
        }
        for y in 4..self.height - 4 {
            for x in 4..self.width - 4 {
                if (y - 4..=y + 4)
                    .any(|yy| (x - 4..=x + 4).any(|xx| !self.valid[yy * self.width + xx]))
                {
                    self.quality[y * self.width + x] = 0;
                }
            }
        }
    }
    pub fn geometry(&self) -> (usize, usize) {
        self.geometry
    }

    /// Mask whole viewport rectangles, including every sampled block that
    /// overlaps an exclusion. UI/avatar masks must be supplied by the caller.
    pub fn capture(
        frame: &Frame<'_>,
        included: Region,
        excluded: &[Region],
    ) -> Result<Self, String> {
        let geometry = (frame.width, frame.height);
        if geometry.0 > 16384
            || geometry.1 > 16384
            || !included.valid(geometry)
            || excluded.len() > 32
            || excluded.iter().any(|r| !r.valid(geometry))
        {
            return Err("Scene viewport or observation mask is invalid.".into());
        }
        let coverage = frame.coverage();
        if included.x < coverage.x
            || included.y < coverage.y
            || included.x + included.width > coverage.x + coverage.width
            || included.y + included.height > coverage.y + coverage.height
        {
            return Err("Scene pixels do not cover the supplied observation mask.".into());
        }
        let step = geometry.0.max(geometry.1).div_ceil(SIDE).max(1);
        let width = geometry.0.div_ceil(step);
        let height = geometry.1.div_ceil(step);
        if width < 24 || height < 24 {
            return Err("Scene viewport is too small for distributed feature matches.".into());
        }
        let mut excluded = excluded.to_vec();
        excluded.sort_by_key(|r| (r.x, r.y, r.width, r.height));
        let mut gray = vec![0u8; width * height];
        let mut valid = vec![false; width * height];
        for y in 0..height {
            for x in 0..width {
                let (left, top) = (x * step, y * step);
                let (right, bottom) = ((left + step).min(geometry.0), (top + step).min(geometry.1));
                if left < included.x
                    || top < included.y
                    || right > included.x + included.width
                    || bottom > included.y + included.height
                    || excluded.iter().any(|r| {
                        left < r.x + r.width && right > r.x && top < r.y + r.height && bottom > r.y
                    })
                {
                    continue;
                }
                // Area averaging limits aliasing when the actual displacement
                // is not an exact multiple of the sampling step.
                let mut sum = 0u64;
                for yy in top..bottom {
                    for xx in left..right {
                        let [r, g, b] = frame
                            .pixel(xx, yy)
                            .ok_or("Scene observation is missing a sampled pixel.")?;
                        sum +=
                            u64::from(77 * u32::from(r) + 150 * u32::from(g) + 29 * u32::from(b));
                    }
                }
                gray[y * width + x] = (sum / ((right - left) * (bottom - top)) as u64 / 256) as u8;
                valid[y * width + x] = true;
            }
        }
        let mut result = Self {
            geometry,
            included,
            excluded,
            width,
            height,
            step,
            descriptors: vec![0; width * height],
            quality: vec![0; width * height],
            features: Vec::new(),
            gray: Vec::new(),
            valid: Vec::new(),
        };
        let mut cells = [None; CELLS * CELLS];
        for y in 4..height - 4 {
            for x in 4..width - 4 {
                let center = gray[y * width + x];
                let (mut bits, mut low, mut high, mut allowed) = (0u64, center, center, true);
                for dy in 0..8 {
                    for dx in 0..8 {
                        let at = (y + dy - 4) * width + x + dx - 4;
                        if !valid[at] {
                            allowed = false;
                            break;
                        }
                        let value = gray[at];
                        low = low.min(value);
                        high = high.max(value);
                        let bit = dy * 8 + dx;
                        let partner = (bit * 37 + 13) % 64;
                        let other = gray[(y + partner / 8 - 4) * width + x + partner % 8 - 4];
                        bits |= u64::from(value > other) << bit;
                    }
                    if !allowed {
                        break;
                    }
                }
                let ones = bits.count_ones();
                if !allowed || high - low < 18 || !(12..=52).contains(&ones) {
                    continue;
                }
                let quality = u16::from(high - low) * (32 - ones.abs_diff(32)) as u16;
                let at = y * width + x;
                result.descriptors[at] = bits;
                result.quality[at] = quality;
                let cell_x = ((x * step - included.x) * CELLS / included.width).min(CELLS - 1);
                let cell_y = ((y * step - included.y) * CELLS / included.height).min(CELLS - 1);
                let cell = cell_y * CELLS + cell_x;
                if cells[cell].is_none_or(|(_, _, best)| quality > best) {
                    cells[cell] = Some((x, y, quality));
                }
            }
        }
        result.features = cells
            .into_iter()
            .flatten()
            .map(|(x, y, _)| (x, y))
            .collect();
        if result.features.len() < MIN_MATCHES {
            return Err("Scene has too little unmasked texture for measured movement.".into());
        }
        result.gray = gray;
        result.valid = valid;
        Ok(result)
    }

    fn best(&self, bits: u64, x: usize, y: usize, radius: usize) -> Option<(usize, usize)> {
        let (mut score, mut best) = (u32::MAX, None);
        for yy in y.saturating_sub(radius).max(4)..=(y + radius).min(self.height - 5) {
            for xx in x.saturating_sub(radius).max(4)..=(x + radius).min(self.width - 5) {
                let at = yy * self.width + xx;
                if self.quality[at] == 0 {
                    continue;
                }
                let cost = (bits ^ self.descriptors[at]).count_ones();
                if cost < score {
                    score = cost;
                    best = Some((xx, yy));
                }
            }
        }
        let (bx, by) = best?;
        if score > 14 {
            return None;
        }
        // A repeated floor pattern must not become an arbitrary chosen offset.
        let mut second = u32::MAX;
        for yy in y.saturating_sub(radius).max(4)..=(y + radius).min(self.height - 5) {
            for xx in x.saturating_sub(radius).max(4)..=(x + radius).min(self.width - 5) {
                if xx.abs_diff(bx).max(yy.abs_diff(by)) <= 2 {
                    continue;
                }
                let at = yy * self.width + xx;
                if self.quality[at] > 0 {
                    second = second.min((bits ^ self.descriptors[at]).count_ones());
                }
            }
        }
        (second >= score.saturating_add(2)).then_some((bx, by))
    }
}

#[derive(Clone, Copy)]
struct Match {
    x: f64,
    y: f64,
    dx: f64,
    dy: f64,
}

fn global_shift(before: &Snapshot, after: &Snapshot, radius: usize) -> Option<(isize, isize)> {
    let side = radius * 2 + 1;
    let mut scores = vec![0u16; side * side];
    let mut best = (u16::MAX, 0isize, 0isize);
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as isize - radius as isize, y as isize - radius as isize);
            let mut score = 0;
            for &(px, py) in &before.features {
                let (xx, yy) = (px as isize + dx, py as isize + dy);
                score += if xx < 0
                    || yy < 0
                    || xx >= after.width as isize
                    || yy >= after.height as isize
                {
                    32
                } else {
                    let at = yy as usize * after.width + xx as usize;
                    if after.quality[at] == 0 {
                        32
                    } else {
                        (before.descriptors[py * before.width + px] ^ after.descriptors[at])
                            .count_ones()
                            .min(24) as u16
                    }
                };
            }
            scores[y * side + x] = score;
            if score < best.0 {
                best = (score, dx, dy);
            }
        }
    }
    let count = before.features.len() as u16;
    if best.0 > count * 14 {
        return None;
    }
    let mut second = u16::MAX;
    for y in 0..side {
        for x in 0..side {
            let (dx, dy) = (x as isize - radius as isize, y as isize - radius as isize);
            if (dx - best.1).abs().max((dy - best.2).abs()) > 2 {
                second = second.min(scores[y * side + x]);
            }
        }
    }
    (second >= best.0.saturating_add(count)).then_some((best.1, best.2))
}

/// No reference is implicitly advanced on failure; the caller decides when a
/// fresh stationary reference is justified. Search and allocations are bounded.
pub fn translation(
    before: &Snapshot,
    after: &Snapshot,
    max_shift_pixels: usize,
) -> Result<Translation, String> {
    if before.geometry != after.geometry
        || before.included != after.included
        || before.excluded != after.excluded
    {
        return Err("Scene geometry or masks changed; capture a new reference.".into());
    }
    let radius = max_shift_pixels.div_ceil(before.step);
    if max_shift_pixels == 0 || radius > SEARCH || radius < 4 {
        return Err("Scene displacement search exceeds its bounded range.".into());
    }
    // Joint alignment disambiguates repeated individual floor tiles; local
    // verification still requires spatially distributed consistent motion.
    let (anchor_x, anchor_y) = global_shift(before, after, radius)
        .ok_or("Scene alignment is insufficient or repeated; movement was not measured.")?;
    let mut matches = Vec::with_capacity(before.features.len());
    for &(x, y) in &before.features {
        let (predicted_x, predicted_y) = (x as isize + anchor_x, y as isize + anchor_y);
        if predicted_x < 4
            || predicted_y < 4
            || predicted_x >= after.width as isize - 4
            || predicted_y >= after.height as isize - 4
        {
            continue;
        }
        let Some((xx, yy)) = after.best(
            before.descriptors[y * before.width + x],
            predicted_x as usize,
            predicted_y as usize,
            2,
        ) else {
            continue;
        };
        let Some((back_x, back_y)) = before.best(after.descriptors[yy * after.width + xx], x, y, 2)
        else {
            continue;
        };
        if back_x.abs_diff(x) > 1 || back_y.abs_diff(y) > 1 {
            continue;
        }
        let (dx, dy) = (xx as f64 - x as f64, yy as f64 - y as f64);
        if dx.abs().max(dy.abs()) >= radius as f64 {
            continue;
        }
        matches.push(Match {
            x: x as f64,
            y: y as f64,
            dx,
            dy,
        });
    }
    if matches.len() < MIN_MATCHES || matches.len() * 5 < before.features.len() * 2 {
        return Err(
            "Scene matches are insufficient or ambiguous; movement was not measured.".into(),
        );
    }
    let close = |a: Match, b: Match| (a.dx - b.dx).hypot(a.dy - b.dy) <= 1.5;
    let center = matches
        .iter()
        .copied()
        .max_by_key(|&candidate| matches.iter().filter(|&&m| close(m, candidate)).count())
        .unwrap();
    let inliers: Vec<_> = matches
        .iter()
        .copied()
        .filter(|&m| close(m, center))
        .collect();
    if inliers.len() < MIN_MATCHES || inliers.len() * 10 < matches.len() * 7 {
        return Err(
            "Scene contains inconsistent motion; fixed-camera translation is unconfirmed.".into(),
        );
    }
    let n = inliers.len() as f64;
    let mean = inliers.iter().fold([0.; 4], |mut s, m| {
        s[0] += m.x;
        s[1] += m.y;
        s[2] += m.dx;
        s[3] += m.dy;
        s
    });
    let [x, y, dx, dy] = mean.map(|v| v / n);
    let (mut min_x, mut max_x, mut min_y, mut max_y) = (f64::INFINITY, 0f64, f64::INFINITY, 0f64);
    let (mut xx, mut xy, mut yy, mut ux, mut uy, mut vx, mut vy) = (0., 0., 0., 0., 0., 0., 0.);
    let mut scatter = 0f64;
    for m in &inliers {
        min_x = min_x.min(m.x);
        max_x = max_x.max(m.x);
        min_y = min_y.min(m.y);
        max_y = max_y.max(m.y);
        let (a, b, u, v) = (m.x - x, m.y - y, m.dx - dx, m.dy - dy);
        xx += a * a;
        xy += a * b;
        yy += b * b;
        ux += a * u;
        uy += b * u;
        vx += a * v;
        vy += b * v;
        scatter = scatter.max(u.hypot(v));
    }
    let (span_x, span_y) = (max_x - min_x, max_y - min_y);
    if span_x * (before.step as f64) < before.included.width as f64 * 0.25
        || span_y * (before.step as f64) < before.included.height as f64 * 0.25
    {
        return Err("Scene matches cover too little area to confirm camera stability.".into());
    }
    let determinant = xx * yy - xy * xy;
    if determinant <= 1.0 {
        return Err("Scene motion is not spatially observable.".into());
    }
    let affine = [
        (ux * yy - uy * xy) / determinant,
        (uy * xx - ux * xy) / determinant,
        (vx * yy - vy * xy) / determinant,
        (vy * xx - vx * xy) / determinant,
    ];
    let tolerance = (1.5 / span_x.min(span_y)).max(0.005);
    if affine.iter().any(|v| v.abs() > tolerance) {
        return Err(
            "Scene scale, rotation or perspective changed; translation is unconfirmed.".into(),
        );
    }
    Ok(Translation {
        dx: dx * before.step as f64,
        dy: dy * before.step as f64,
        error_pixels: (scatter + 1.) * before.step as f64,
        matches: inliers.len(),
        features: before.features.len(),
    })
}

/// A measured affine mapping of one visible local image plane. This is not a
/// world pose or evidence that the camera stayed fixed. Callers must inspect
/// scale/rotation and keep mapped points inside the observed support.
#[derive(Clone, Debug)]
pub struct Registration {
    /// Viewport pixels: [x', y'] = affine * [x, y, 1].
    pub affine: [[f64; 3]; 2],
    /// Maximum inlier residual plus sampling uncertainty, in image pixels.
    pub error_pixels: f64,
    pub matches: usize,
    pub features: usize,
    pub scale: [f64; 2],
    pub rotation_radians: f64,
    /// Bounding rectangle of source inliers; it does not assert ground height.
    pub support: [f64; 4],
    /// Convex interpolation support in the source image, in viewport pixels.
    pub support_hull: Vec<[f64; 2]>,
}

impl Registration {
    pub(crate) fn map_vector(&self, p: [f64; 2]) -> [f64; 2] {
        self.affine.map(|r| r[0] * p[0] + r[1] * p[1])
    }
    pub(crate) fn inverse_point(&self, p: [f64; 2]) -> Option<[f64; 2]> {
        let a = self.affine;
        let d = a[0][0] * a[1][1] - a[0][1] * a[1][0];
        if !p.into_iter().all(f64::is_finite) || !d.is_finite() || d <= 0.5 {
            return None;
        }
        let (x, y) = (p[0] - a[0][2], p[1] - a[1][2]);
        let q = [
            (a[1][1] * x - a[0][1] * y) / d,
            (-a[1][0] * x + a[0][0] * y) / d,
        ];
        self.supports(q).then_some(q)
    }
    pub fn map_point(&self, point: [f64; 2]) -> [f64; 2] {
        self.affine
            .map(|r| r[0] * point[0] + r[1] * point[1] + r[2])
    }

    /// Boundary points are supported; extrapolation and non-finite points are not.
    pub fn supports(&self, point: [f64; 2]) -> bool {
        point.iter().all(|v| v.is_finite())
            && self.support_hull.len() >= 3
            && (0..self.support_hull.len()).all(|i| {
                let a = self.support_hull[i];
                let b = self.support_hull[(i + 1) % self.support_hull.len()];
                cross(a, b, point) >= -1e-7
            })
    }
}

fn cross(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn support_hull(mut points: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    points.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    let chain = |points: Vec<[f64; 2]>| {
        let mut hull: Vec<[f64; 2]> = Vec::new();
        for point in points {
            while hull.len() >= 2 && cross(hull[hull.len() - 2], hull[hull.len() - 1], point) <= 0.
            {
                hull.pop();
            }
            hull.push(point);
        }
        hull.pop();
        hull
    };
    let mut hull = chain(points.clone());
    points.reverse();
    hull.extend(chain(points));
    hull
}

fn registration_features(snapshot: &Snapshot, region: Region) -> Vec<(usize, usize)> {
    let mut cells: [Vec<(u16, usize, usize)>; CELLS * CELLS] = std::array::from_fn(|_| Vec::new());
    for y in 5..snapshot.height - 5 {
        for x in 5..snapshot.width - 5 {
            let q = snapshot.quality[y * snapshot.width + x];
            let (px, py) = (x * snapshot.step, y * snapshot.step);
            if q == 0
                || px < region.x
                || py < region.y
                || px >= region.x + region.width
                || py >= region.y + region.height
            {
                continue;
            }
            if region != snapshot.included
                && ((x - 4) * snapshot.step < region.x
                    || (y - 4) * snapshot.step < region.y
                    || (x + 5) * snapshot.step > region.x + region.width
                    || (y + 5) * snapshot.step > region.y + region.height)
            {
                continue;
            }
            let cx = ((px - region.x) * CELLS / region.width).min(CELLS - 1);
            let cy = ((py - region.y) * CELLS / region.height).min(CELLS - 1);
            let cell = &mut cells[cy * CELLS + cx];
            if let Some(index) = cell
                .iter()
                .position(|&(_, px, py)| x.abs_diff(px).max(y.abs_diff(py)) < 6)
            {
                if cell[index].0 >= q {
                    continue;
                }
                cell.remove(index);
            }
            cell.push((q, x, y));
            cell.sort_unstable_by(|a, b| b.0.cmp(&a.0));
            cell.truncate(4);
        }
    }
    let mut candidates: Vec<_> = cells.into_iter().flatten().collect();
    if region == snapshot.included {
        return candidates.into_iter().map(|(_, x, y)| (x, y)).collect();
    }
    // Local cells can be smaller than a descriptor. Do not count adjacent,
    // almost-identical windows as independent evidence for a small plane.
    candidates.sort_unstable_by(|a, b| b.cmp(a));
    let mut distributed: Vec<(usize, usize)> = Vec::new();
    for (_, x, y) in candidates {
        if distributed
            .iter()
            .all(|&(u, v)| x.abs_diff(u).max(y.abs_diff(v)) >= 6)
        {
            distributed.push((x, y));
        }
    }
    distributed
}

fn correlation(a: &Snapshot, x: usize, y: usize, b: &Snapshot, u: usize, v: usize) -> Option<f64> {
    let (mut sum_a, mut sum_b, mut aa, mut bb, mut ab) = (0., 0., 0., 0., 0.);
    for yy in 0..9 {
        for xx in 0..9 {
            let i = (y + yy - 4) * a.width + x + xx - 4;
            let j = (v + yy - 4) * b.width + u + xx - 4;
            if !a.valid[i] || !b.valid[j] {
                return None;
            }
            let (p, q) = (f64::from(a.gray[i]), f64::from(b.gray[j]));
            sum_a += p;
            sum_b += q;
            aa += p * p;
            bb += q * q;
            ab += p * q;
        }
    }
    let (aa, bb) = (aa - sum_a * sum_a / 81., bb - sum_b * sum_b / 81.);
    (aa >= 81. * 16. && bb >= 81. * 16.)
        .then(|| ((ab - sum_a * sum_b / 81.) / (aa * bb).sqrt()).clamp(-1., 1.))
}

fn feature_match(
    source: &Snapshot,
    target: &Snapshot,
    x: usize,
    y: usize,
    radius: usize,
) -> Option<(usize, usize)> {
    let bits = source.descriptors[y * source.width + x];
    // Census shortlisting bounds the expensive photometric verification.
    let mut candidates = Vec::<(u32, usize, usize)>::with_capacity(25);
    for v in y.saturating_sub(radius).max(5)..=(y + radius).min(target.height - 6) {
        for u in x.saturating_sub(radius).max(5)..=(x + radius).min(target.width - 6) {
            let at = v * target.width + u;
            if target.quality[at] == 0 {
                continue;
            }
            let cost = (bits ^ target.descriptors[at]).count_ones();
            if cost > 18 || candidates.len() == 24 && cost > candidates[23].0 {
                continue;
            }
            candidates.push((cost, u, v));
            candidates.sort_unstable();
            candidates.truncate(24);
        }
    }
    let mut verified: Vec<_> = candidates
        .into_iter()
        .filter_map(|(_, u, v)| correlation(source, x, y, target, u, v).map(|n| (n, u, v)))
        .collect();
    verified.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
    let &(score, u, v) = verified.first()?;
    if score < 0.78
        || verified
            .iter()
            .skip(1)
            .any(|&(n, xx, yy)| u.abs_diff(xx).max(v.abs_diff(yy)) > 2 && n > score - 0.04)
    {
        return None;
    }
    Some((u, v))
}

/// Verify actual textured pixels at an already measured plane projection.
/// This can recover a floor texture cell rejected by a conservative color
/// flood, but cannot fill a flat/occluded/unmatched hole just from a hull.
pub(crate) fn verify_projection(
    before: &Snapshot,
    after: &Snapshot,
    plane: &Registration,
    point: [f64; 2],
) -> bool {
    if before.geometry != after.geometry || before.step != after.step || !plane.supports(point) {
        return false;
    }
    let step = before.step as f64;
    let source = [
        (point[0] / step).round() as usize,
        (point[1] / step).round() as usize,
    ];
    let predicted = plane.map_point([source[0] as f64 * step, source[1] as f64 * step]);
    let target = [
        (predicted[0] / step).round() as usize,
        (predicted[1] / step).round() as usize,
    ];
    let radius = (plane.error_pixels / step).ceil().clamp(1., 4.) as usize;
    let Some(found) = near_patch(before, source, after, target, radius) else {
        return false;
    };
    if (found[0] as f64 * step - predicted[0]).hypot(found[1] as f64 * step - predicted[1])
        > plane.error_pixels + step
    {
        return false;
    }
    near_patch(after, found, before, source, radius)
        .is_some_and(|back| back[0].abs_diff(source[0]).max(back[1].abs_diff(source[1])) <= 1)
}

fn near_patch(
    source: &Snapshot,
    at: [usize; 2],
    target: &Snapshot,
    center: [usize; 2],
    radius: usize,
) -> Option<[usize; 2]> {
    if at[0] < 5 || at[1] < 5 || at[0] + 5 >= source.width || at[1] + 5 >= source.height {
        return None;
    }
    let mut values = Vec::new();
    for y in center[1].saturating_sub(radius).max(5)..=(center[1] + radius).min(target.height - 6) {
        for x in
            center[0].saturating_sub(radius).max(5)..=(center[0] + radius).min(target.width - 6)
        {
            if let Some(score) = correlation(source, at[0], at[1], target, x, y) {
                values.push((score, [x, y]));
            }
        }
    }
    values.sort_unstable_by(|a, b| b.0.total_cmp(&a.0));
    let &(score, point) = values.first()?;
    if score < 0.80
        || values.iter().skip(1).any(|(s, p)| {
            p[0].abs_diff(point[0]).max(p[1].abs_diff(point[1])) > 1 && *s > score - 0.04
        })
    {
        return None;
    }
    Some(point)
}

fn fit_affine(matches: &[Match]) -> Option<[[f64; 3]; 2]> {
    let n = matches.len() as f64;
    if n < 3. {
        return None;
    }
    let mut mean = [0.; 4];
    for m in matches {
        for (s, p) in mean.iter_mut().zip([m.x, m.y, m.x + m.dx, m.y + m.dy]) {
            *s += p / n;
        }
    }
    let [x, y, u, v] = mean;
    let (mut xx, mut xy, mut yy, mut xu, mut yu, mut xv, mut yv) = (0., 0., 0., 0., 0., 0., 0.);
    for m in matches {
        let (a, b, c, d) = (m.x - x, m.y - y, m.x + m.dx - u, m.y + m.dy - v);
        xx += a * a;
        xy += a * b;
        yy += b * b;
        xu += a * c;
        yu += b * c;
        xv += a * d;
        yv += b * d;
    }
    let det = xx * yy - xy * xy;
    if det <= 1. || det < xx * yy * 0.001 {
        return None;
    }
    let (a, b, c, d) = (
        (xu * yy - yu * xy) / det,
        (yu * xx - xu * xy) / det,
        (xv * yy - yv * xy) / det,
        (yv * xx - xv * xy) / det,
    );
    Some([[a, b, u - a * x - b * y], [c, d, v - c * x - d * y]])
}

fn residual(model: [[f64; 3]; 2], m: Match) -> f64 {
    (model[0][0] * m.x + model[0][1] * m.y + model[0][2] - m.x - m.dx)
        .hypot(model[1][0] * m.x + model[1][1] * m.y + model[1][2] - m.y - m.dy)
}

/// Estimate a bounded local affine image transform from independently matched,
/// bidirectionally verified texture. Repeated/unsupported observations fail;
/// accepting a transform never authorizes a world movement or camera model.
pub fn register(
    before: &Snapshot,
    after: &Snapshot,
    max_shift_pixels: usize,
) -> Result<Registration, String> {
    register_local(before, after, max_shift_pixels, before.included)
}

/// Independently register one actually visible source patch. Features are
/// distributed within this ROI rather than the entire viewport, so a smaller
/// textured landing plane can be measured separately from its surroundings.
/// The ROI selects evidence; the result supports ONLY its observed inlier hull.
pub(crate) fn register_local(
    before: &Snapshot,
    after: &Snapshot,
    max_shift_pixels: usize,
    source_region: Region,
) -> Result<Registration, String> {
    register_plane(before, after, max_shift_pixels, source_region, None)
}

/// Keep the actually observed foot-floor plane as the reference even when a
/// different, raised surface occupies more image pixels. Only independently
/// matched pixels may extend the seed's support hull; the seed is never used
/// as evidence for an unobserved extrapolated point.
pub(crate) fn register_anchored(
    before: &Snapshot,
    after: &Snapshot,
    max_shift_pixels: usize,
    anchor: Region,
) -> Result<Registration, String> {
    let seed = register_local(before, after, max_shift_pixels, anchor)?;
    let center = [
        (anchor.x + anchor.width / 2) as f64,
        (anchor.y + anchor.height / 2) as f64,
    ];
    if !seed.supports(center) {
        return Err("The observed foot-floor anchor has no surrounding plane support.".into());
    }
    register_plane(
        before,
        after,
        max_shift_pixels,
        before.included,
        Some((&seed, anchor)),
    )
}

fn register_plane(
    before: &Snapshot,
    after: &Snapshot,
    max_shift_pixels: usize,
    source_region: Region,
    anchor: Option<(&Registration, Region)>,
) -> Result<Registration, String> {
    if before.geometry != after.geometry
        || before.included != after.included
        || before.excluded != after.excluded
    {
        return Err("Scene geometry or masks changed; capture a new reference.".into());
    }
    if !source_region.valid(before.geometry)
        || source_region.x < before.included.x
        || source_region.y < before.included.y
        || source_region.x + source_region.width > before.included.x + before.included.width
        || source_region.y + source_region.height > before.included.y + before.included.height
        || (source_region != before.included
            && (source_region.width < before.step * 12 || source_region.height < before.step * 12))
    {
        return Err("Scene local plane ROI is outside observed pixels or too small.".into());
    }
    let radius = max_shift_pixels.div_ceil(before.step);
    if max_shift_pixels == 0 || !(4..=SEARCH).contains(&radius) {
        return Err("Scene displacement search exceeds its bounded range.".into());
    }
    let mut features = registration_features(before, source_region);
    if let Some((_, region)) = anchor {
        features.extend(registration_features(before, region));
        features.sort_unstable();
        features.dedup();
    }
    let mut matches = Vec::with_capacity(features.len());
    for &(x, y) in &features {
        let Some((u, v)) = feature_match(before, after, x, y, radius) else {
            continue;
        };
        let Some((xx, yy)) = feature_match(after, before, u, v, radius) else {
            continue;
        };
        if xx.abs_diff(x).max(yy.abs_diff(y)) > 1 {
            continue;
        }
        matches.push(Match {
            x: x as f64,
            y: y as f64,
            dx: u as f64 - x as f64,
            dy: v as f64 - y as f64,
        });
    }
    if matches.len() < 12 || matches.len() * 8 < features.len() {
        return Err("Scene affine features are insufficient or ambiguous.".into());
    }
    const TOLERANCE: f64 = 1.75;
    if let Some((seed, _)) = anchor {
        let mut model = seed.affine;
        for row in &mut model {
            row[2] /= before.step as f64;
        }
        // Extend by VERIFIED correspondences agreeing with the local witness,
        // never by whichever unrelated plane has the larger pixel count.
        matches.retain(|m| residual(model, *m) <= TOLERANCE);
        if matches.len() < 12 {
            return Err("The anchored foot plane has too few consistent correspondences.".into());
        }
    }
    let agrees = |model: [[f64; 3]; 2]| {
        anchor.is_none_or(|(seed, _)| {
            seed.support_hull.iter().all(|p| {
                let actual = model.map(|r| {
                    (r[0] * (p[0] / before.step as f64) + r[1] * (p[1] / before.step as f64) + r[2])
                        * before.step as f64
                });
                let expected = seed.map_point(*p);
                (actual[0] - expected[0]).hypot(actual[1] - expected[1])
                    <= seed.error_pixels + TOLERANCE * before.step as f64
            })
        })
    };
    let mut best = if let Some((seed, _)) = anchor {
        let mut model = seed.affine;
        for r in &mut model {
            r[2] /= before.step as f64;
        }
        matches
            .iter()
            .copied()
            .filter(|m| residual(model, *m) <= TOLERANCE)
            .collect()
    } else {
        Vec::new()
    };
    let mut best_error = f64::INFINITY;
    let mut seed = 0x6ac1e57du32;
    // Deterministic bounded RANSAC; no candidate or input state survives failure.
    for _ in 0..768 {
        let mut sample = [matches[0]; 3];
        for m in &mut sample {
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            *m = matches[seed as usize % matches.len()];
        }
        let Some(model) = fit_affine(&sample) else {
            continue;
        };
        if !agrees(model) {
            continue;
        }
        let inliers: Vec<_> = matches
            .iter()
            .copied()
            .filter(|&m| residual(model, m) <= TOLERANCE)
            .collect();
        let error: f64 = inliers.iter().map(|&m| residual(model, m)).sum();
        if inliers.len() > best.len() || inliers.len() == best.len() && error < best_error {
            best = inliers;
            best_error = error;
        }
    }
    if best.len() < 12 || anchor.is_none() && best.len() * 5 < matches.len() * 3 {
        return Err("No dominant local image plane was measured.".into());
    }
    let mut model = fit_affine(&best).ok_or("Scene affine plane is not observable.")?;
    // Refit with every consensus point, retaining the same bounded error gate.
    best.retain(|&m| residual(model, m) <= TOLERANCE);
    if best.len() < 12 || anchor.is_none() && best.len() * 5 < matches.len() * 3 {
        return Err("Scene affine consensus did not survive verification.".into());
    }
    model = fit_affine(&best).ok_or("Scene affine plane is not observable.")?;
    if !agrees(model) {
        return Err("Global plane consensus left the measured foot-floor anchor.".into());
    }
    let mut support = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    let mut cells = [false; CELLS * CELLS];
    let mut error = 0f64;
    for m in &best {
        support[0] = support[0].min(m.x);
        support[1] = support[1].min(m.y);
        support[2] = support[2].max(m.x);
        support[3] = support[3].max(m.y);
        let cx = ((m.x as usize * before.step - source_region.x) * CELLS / source_region.width)
            .min(CELLS - 1);
        let cy = ((m.y as usize * before.step - source_region.y) * CELLS / source_region.height)
            .min(CELLS - 1);
        cells[cy * CELLS + cx] = true;
        error = error.max(residual(model, *m));
    }
    let step = before.step as f64;
    let distribution = anchor.map_or(source_region, |(_, region)| region);
    if let Some((_, region)) = anchor {
        let witnesses: Vec<_> = best
            .iter()
            .filter(|m| {
                m.x * step >= region.x as f64
                    && m.y * step >= region.y as f64
                    && m.x * step <= (region.x + region.width) as f64
                    && m.y * step <= (region.y + region.height) as f64
            })
            .map(|m| [m.x * step, m.y * step])
            .collect();
        let hull = support_hull(witnesses.clone());
        let center = [
            (region.x + region.width / 2) as f64,
            (region.y + region.height / 2) as f64,
        ];
        if witnesses.len() < 8
            || hull.len() < 3
            || !(0..hull.len()).all(|i| cross(hull[i], hull[(i + 1) % hull.len()], center) >= -1e-7)
        {
            return Err("The extended image plane lost its actual local anchor witnesses.".into());
        }
    }
    if (support[2] - support[0]) * step < distribution.width as f64 * 0.25
        || (support[3] - support[1]) * step < distribution.height as f64 * 0.25
        || anchor.is_none() && cells.into_iter().filter(|&v| v).count() < 6
    {
        return Err("Scene affine matches are not spatially distributed.".into());
    }
    let scale = [
        model[0][0].hypot(model[1][0]),
        model[0][1].hypot(model[1][1]),
    ];
    let determinant = model[0][0] * model[1][1] - model[0][1] * model[1][0];
    let rotation_radians = model[1][0].atan2(model[0][0]);
    if scale.iter().any(|s| !(0.8..=1.25).contains(s))
        || !(0.65..=1.6).contains(&determinant)
        || rotation_radians.abs() > 0.3
        || error > TOLERANCE
    {
        return Err("Scene affine scale, rotation or residual exceeds its measured range.".into());
    }
    for row in &mut model {
        row[2] *= step;
    }
    Ok(Registration {
        affine: model,
        error_pixels: (error + 1.) * step,
        matches: best.len(),
        features: features.len(),
        scale,
        rotation_radians,
        support: support.map(|v| v * step),
        support_hull: support_hull(best.iter().map(|m| [m.x * step, m.y * step]).collect()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    const W: usize = 160;
    const H: usize = 128;
    fn texture() -> Vec<u8> {
        let mut bytes = vec![0; W * H * 4];
        let mut seed = 17u32;
        for p in bytes.chunks_exact_mut(4) {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            let v = 40 + (seed % 160) as u8;
            p.copy_from_slice(&[v, v, v, 255]);
        }
        bytes
    }
    fn moved(source: &[u8], dx: isize, dy: isize, brightness: u8) -> Vec<u8> {
        let mut bytes = vec![0; source.len()];
        for y in 0..H {
            for x in 0..W {
                let (sx, sy) = (x as isize - dx, y as isize - dy);
                if sx >= 0 && sy >= 0 && sx < W as isize && sy < H as isize {
                    let p = (y * W + x) * 4;
                    let q = (sy as usize * W + sx as usize) * 4;
                    for c in 0..3 {
                        bytes[p + c] = source[q + c] + brightness;
                    }
                    bytes[p + 3] = 255;
                }
            }
        }
        bytes
    }
    fn snapshot(bytes: &[u8], excluded: &[Region]) -> Snapshot {
        Snapshot::capture(
            &Frame::new(bytes, W, H, W * 4, 4, false).unwrap(),
            Region::full(W, H),
            excluded,
        )
        .unwrap()
    }
    #[test]
    fn measures_translation_despite_brightness_and_partial_occlusion() {
        let before = texture();
        let mut after = moved(&before, 7, -5, 12);
        for y in 45..80 {
            for x in 60..90 {
                after[(y * W + x) * 4..(y * W + x) * 4 + 4].fill(0);
            }
        }
        let measurement = translation(&snapshot(&before, &[]), &snapshot(&after, &[]), 20).unwrap();
        assert!((measurement.dx - 7.).abs() < 0.2 && (measurement.dy + 5.).abs() < 0.2);
        assert!(measurement.matches >= MIN_MATCHES && measurement.error_pixels <= 2.);
    }
    #[test]
    fn excludes_a_stationary_foreground_in_both_images() {
        let before = texture();
        let mut after = moved(&before, -6, 4, 0);
        let mask = Region {
            x: 25,
            y: 25,
            width: 45,
            height: 65,
        };
        for y in mask.y..mask.y + mask.height {
            for x in mask.x..mask.x + mask.width {
                let p = (y * W + x) * 4;
                after[p..p + 4].copy_from_slice(&before[p..p + 4]);
            }
        }
        let measurement =
            translation(&snapshot(&before, &[mask]), &snapshot(&after, &[mask]), 20).unwrap();
        assert_eq!((measurement.dx, measurement.dy), (-6., 4.));
    }
    #[test]
    fn distributes_features_within_a_small_observed_patch() {
        let before = texture();
        let after = moved(&before, 3, -2, 0);
        let region = Region {
            x: 40,
            y: 32,
            width: 32,
            height: 64,
        };
        let capture = |bytes: &[u8]| {
            Snapshot::capture(
                &Frame::new(bytes, W, H, W * 4, 4, false).unwrap(),
                region,
                &[],
            )
            .unwrap()
        };
        let measurement = translation(&capture(&before), &capture(&after), 12).unwrap();
        assert_eq!((measurement.dx, measurement.dy), (3., -2.));
        assert!(measurement.matches >= MIN_MATCHES);
    }

    #[test]
    fn measures_displacement_between_downsampled_pixel_phases() {
        let (w, h) = (640usize, 320usize);
        let mut before = vec![0u8; w * h * 4];
        let value = |x: usize, y: usize| {
            let mut n = (x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663);
            n ^= n >> 13;
            40 + n % 160
        };
        for y in 0..h {
            for x in 0..w {
                let (a, b, fx, fy) = (x / 4, y / 4, x % 4, y % 4);
                let top = value(a, b) * (4 - fx) as u32 + value(a + 1, b) * fx as u32;
                let bottom = value(a, b + 1) * (4 - fx) as u32 + value(a + 1, b + 1) * fx as u32;
                let v = ((top * (4 - fy) as u32 + bottom * fy as u32) / 16) as u8;
                before[(y * w + x) * 4..(y * w + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let mut after = vec![0; before.len()];
        for y in 0..h - 3 {
            for x in 3..w {
                let p = (y * w + x) * 4;
                let q = ((y + 3) * w + x - 3) * 4;
                after[p..p + 4].copy_from_slice(&before[q..q + 4]);
            }
        }
        let capture = |bytes: &[u8]| {
            Snapshot::capture(
                &Frame::new(bytes, w, h, w * 4, 4, false).unwrap(),
                Region::full(w, h),
                &[],
            )
            .unwrap()
        };
        let measured = translation(&capture(&before), &capture(&after), 32).unwrap();
        assert!((measured.dx - 3.).hypot(measured.dy + 3.) <= measured.error_pixels);
        assert!(measured.dx > 0. && measured.dy < 0.);
    }
    #[test]
    fn rejects_blank_repeated_and_unrelated_images() {
        let blank = vec![80; W * H * 4];
        assert!(Snapshot::capture(
            &Frame::new(&blank, W, H, W * 4, 4, false).unwrap(),
            Region::full(W, H),
            &[]
        )
        .is_err());
        let mut repeated = vec![0; W * H * 4];
        for y in 0..H {
            for x in 0..W {
                let p = (y * W + x) * 4;
                let v = if (x / 4 + y / 4) % 2 == 0 { 40 } else { 190 };
                repeated[p..p + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let repeated = snapshot(&repeated, &[]);
        assert!(translation(&repeated, &repeated, 20).is_err());
        let a = texture();
        let mut b = a.clone();
        b.reverse();
        assert!(translation(&snapshot(&a, &[]), &snapshot(&b, &[]), 20).is_err());
    }
    #[test]
    fn rejects_changed_masks_zoom_and_conflicting_motion() {
        let before = texture();
        let first = snapshot(&before, &[]);
        let resized_frame = Frame::new(&before, W - 1, H, W * 4, 4, false).unwrap();
        let resized = Snapshot::capture(&resized_frame, Region::full(W - 1, H), &[]).unwrap();
        assert!(translation(&first, &resized, 20).is_err());
        let excluded = Region {
            x: 10,
            y: 10,
            width: 15,
            height: 15,
        };
        assert!(translation(&first, &snapshot(&before, &[excluded]), 20).is_err());
        let mut zoomed = vec![0; before.len()];
        for y in 0..H {
            for x in 0..W {
                let sx = ((x as f64 - W as f64 / 2.) / 1.08 + W as f64 / 2.) as usize;
                let sy = ((y as f64 - H as f64 / 2.) / 1.08 + H as f64 / 2.) as usize;
                let p = (y * W + x) * 4;
                let q = (sy * W + sx) * 4;
                zoomed[p..p + 4].copy_from_slice(&before[q..q + 4]);
            }
        }
        assert!(translation(&first, &snapshot(&zoomed, &[]), 20).is_err());
        let mut split = moved(&before, 7, 0, 0);
        let other = moved(&before, -7, 0, 0);
        split[..W * H * 2].copy_from_slice(&other[..W * H * 2]);
        assert!(translation(&first, &snapshot(&split, &[]), 20).is_err());
    }

    #[test]
    fn rejects_camera_rotation_instead_of_reporting_a_pose() {
        let before = texture();
        let mut rotated = vec![0; before.len()];
        let (cos, sin) = (0.08f64.cos(), 0.08f64.sin());
        for y in 0..H {
            for x in 0..W {
                let (a, b) = (x as f64 - W as f64 / 2., y as f64 - H as f64 / 2.);
                let sx = (a * cos + b * sin + W as f64 / 2.).round() as isize;
                let sy = (-a * sin + b * cos + H as f64 / 2.).round() as isize;
                if sx >= 0 && sy >= 0 && sx < W as isize && sy < H as isize {
                    let p = (y * W + x) * 4;
                    let q = (sy as usize * W + sx as usize) * 4;
                    rotated[p..p + 4].copy_from_slice(&before[q..q + 4]);
                }
            }
        }
        assert!(translation(&snapshot(&before, &[]), &snapshot(&rotated, &[]), 20).is_err());
    }

    fn affine_image(source: &[u8], model: [[f64; 3]; 2]) -> Vec<u8> {
        let mut bytes = vec![0; source.len()];
        let det = model[0][0] * model[1][1] - model[0][1] * model[1][0];
        for y in 0..H {
            for x in 0..W {
                let (u, v) = (x as f64 - model[0][2], y as f64 - model[1][2]);
                let sx = ((model[1][1] * u - model[0][1] * v) / det).round() as isize;
                let sy = ((-model[1][0] * u + model[0][0] * v) / det).round() as isize;
                if sx >= 0 && sy >= 0 && sx < W as isize && sy < H as isize {
                    let p = (y * W + x) * 4;
                    let q = (sy as usize * W + sx as usize) * 4;
                    bytes[p..p + 4].copy_from_slice(&source[q..q + 4]);
                }
            }
        }
        bytes
    }

    #[test]
    fn affine_registration_reports_scale_rotation_and_local_point_mapping() {
        let before = texture();
        let model = [[1.025, -0.025, 3.], [0.03, 1.01, -4.]];
        let mut after = affine_image(&before, model);
        for y in 45..70 {
            for x in 65..90 {
                after[(y * W + x) * 4..(y * W + x) * 4 + 4].fill(0);
            }
        }
        let registration = register(&snapshot(&before, &[]), &snapshot(&after, &[]), 20).unwrap();
        for point in [[30., 30.], [120., 95.]] {
            let expected = model.map(|r| r[0] * point[0] + r[1] * point[1] + r[2]);
            let measured = registration.map_point(point);
            assert!((measured[0] - expected[0]).hypot(measured[1] - expected[1]) < 1.);
        }
        assert!(registration.scale[0] > 1.01);
        assert!(registration.rotation_radians > 0.015);
        assert!(registration.matches >= 12 && registration.error_pixels < 3.);
        // A successful affine image fit must not relax fixed-camera translation.
        assert!(translation(&snapshot(&before, &[]), &snapshot(&after, &[]), 20).is_err());
    }

    #[test]
    fn affine_registration_is_zero_for_static_texture_and_rejects_ambiguous_planes() {
        let before = texture();
        let reference = snapshot(&before, &[]);
        let static_fit = register(&reference, &reference, 20).unwrap();
        assert_eq!(static_fit.map_point([50., 60.]), [50., 60.]);
        let mut split = moved(&before, 7, 0, 0);
        let other = moved(&before, -7, 0, 0);
        split[..W * H * 2].copy_from_slice(&other[..W * H * 2]);
        assert!(register(&reference, &snapshot(&split, &[]), 20).is_err());
        let mut unrelated = before.clone();
        unrelated.reverse();
        assert!(register(&reference, &snapshot(&unrelated, &[]), 20).is_err());

        let mut repeated = vec![0; before.len()];
        for y in 0..H {
            for x in 0..W {
                let v = if (x / 4 + y / 4) % 2 == 0 { 40 } else { 190 };
                repeated[(y * W + x) * 4..(y * W + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
            }
        }
        let repeated = snapshot(&repeated, &[]);
        assert!(register(&repeated, &repeated, 20).is_err());
    }

    #[test]
    fn affine_support_excludes_unobserved_bbox_corners_and_includes_its_boundary() {
        let registration = Registration {
            affine: [[1., 0., 0.], [0., 1., 0.]],
            error_pixels: 1.,
            matches: 12,
            features: 12,
            scale: [1., 1.],
            rotation_radians: 0.,
            support: [0., 0., 10., 10.],
            support_hull: support_hull(vec![
                [0., 0.],
                [10., 0.],
                [0., 10.],
                [5., 5.],
                [0., 0.],
                [3., 0.],
            ]),
        };
        assert_eq!(registration.support_hull.len(), 3);
        for point in [[2., 3.], [0., 0.], [10., 0.], [5., 5.], [0., 5.]] {
            assert!(registration.supports(point), "{point:?}");
        }
        for point in [[8., 8.], [5., -0.01], [f64::NAN, 2.], [f64::INFINITY, 0.]] {
            assert!(!registration.supports(point), "{point:?}");
        }
    }

    #[test]
    fn moving_body_masks_compare_the_union_then_restore_only_newly_visible_pixels() {
        let before = texture();
        let after = moved(&before, -5, 3, 0);
        let old = Region {
            x: 40,
            y: 45,
            width: 18,
            height: 26,
        };
        let current = Region {
            x: 75,
            y: 40,
            width: 18,
            height: 26,
        };
        let mut a = snapshot(&before, &[old]);
        a.exclude_region(current).unwrap();
        let b = snapshot(&after, &[old, current]);
        let mapping = register(&a, &b, 20).unwrap();
        let point = [80., 90.];
        let found = mapping.map_point(point);
        assert!((found[0] - point[0] + 5.).hypot(found[1] - point[1] - 3.) < 1.);
        assert!(
            !a.valid[50 * a.width + 45],
            "adding a mask must not restore any old unknown pixel"
        );
        let next = snapshot(&after, &[current]);
        assert!(
            next.valid[50 * next.width + 45],
            "the fresh frame may restore ground that is really visible now"
        );
        assert!(!next.valid[50 * next.width + 80]);
    }

    #[test]
    fn a_small_local_plane_is_measured_independently_of_the_dominant_base_plane() {
        let before = texture();
        let mut after = moved(&before, 3, 2, 0);
        let patch = Region {
            x: 88,
            y: 35,
            width: 48,
            height: 42,
        };
        // The smaller plane has a different vertical image motion. It remains
        // actual pixels, not a supplied height or an inferred world coordinate.
        for y in patch.y..patch.y + patch.height {
            for x in patch.x..patch.x + patch.width {
                let source = (y * W + x) * 4;
                let target = ((y - 5) * W + x + 3) * 4;
                after[target..target + 4].copy_from_slice(&before[source..source + 4]);
            }
        }
        let a = snapshot(&before, &[]);
        let b = snapshot(&after, &[]);
        let global = register(&a, &b, 20).unwrap();
        let local = register_local(&a, &b, 20, patch).unwrap();
        let p = [112., 56.];
        let base = global.map_point(p);
        let raised = local.map_point(p);
        assert!((base[0] - 115.).hypot(base[1] - 58.) < 1.);
        assert!((raised[0] - 115.).hypot(raised[1] - 51.) < 1.);
        assert!(local.matches >= 12 && local.error_pixels <= 2.);
        assert!(local.supports(p));
        assert!(
            !local.supports([40., 56.]),
            "a local fit never extends into an unobserved plane"
        );

        // Per-frame surface masks can select the same moving plane with
        // different pixel footprints; correspondence is still independently
        // checked in both directions.
        let mut a = snapshot(&before, &[]);
        let mut b = snapshot(&after, &[]);
        a.restrict_surface(|x, y| {
            x >= patch.x && x < patch.x + patch.width && y >= patch.y && y < patch.y + patch.height
        });
        b.restrict_surface(|x, y| {
            x >= patch.x + 3
                && x < patch.x + patch.width + 3
                && y >= patch.y - 5
                && y < patch.y + patch.height - 5
        });
        let masked = register_local(&a, &b, 20, patch).unwrap();
        assert!(masked.supports(p));
        assert!((masked.map_point(p)[1] - 51.).abs() < 1.);
    }

    #[test]
    fn local_planes_reject_holes_ambiguity_and_unobserved_or_too_small_regions() {
        let before = texture();
        let patch = Region {
            x: 88,
            y: 35,
            width: 48,
            height: 42,
        };
        let mut masked = snapshot(&before, &[]);
        masked.restrict_surface(|x, y| {
            x < patch.x || x >= patch.x + patch.width || y < patch.y || y >= patch.y + patch.height
        });
        assert!(register_local(&snapshot(&before, &[]), &masked, 20, patch).is_err());

        for repeated in [false, true] {
            let mut image = before.clone();
            for y in patch.y..patch.y + patch.height {
                for x in patch.x..patch.x + patch.width {
                    let v = if !repeated {
                        90
                    } else if (x / 4 + y / 4) % 2 == 0 {
                        40
                    } else {
                        190
                    };
                    image[(y * W + x) * 4..(y * W + x) * 4 + 4].copy_from_slice(&[v, v, v, 255]);
                }
            }
            let a = snapshot(&image, &[]);
            assert!(
                register_local(&a, &a, 20, patch).is_err(),
                "flat or repeated pixels cannot prove a landing plane"
            );
        }
        let a = snapshot(&before, &[]);
        for region in [
            Region {
                x: W - 10,
                y: 30,
                width: 20,
                height: 30,
            },
            Region {
                x: 40,
                y: 40,
                width: 11,
                height: 40,
            },
            Region {
                x: 40,
                y: 40,
                width: 14,
                height: 14,
            },
        ] {
            assert!(register_local(&a, &a, 20, region).is_err());
        }
    }

    #[test]
    fn the_observed_foot_plane_stays_anchored_when_another_plane_competes() {
        let detail = texture();
        let mut base = detail.clone();
        for p in base.chunks_exact_mut(4) {
            let v = 80 + p[0] % 40;
            p[..3].fill(v);
        }
        let patch = Region {
            x: 40,
            y: 18,
            width: 100,
            height: 90,
        };
        let mut before = base.clone();
        for y in patch.y..patch.y + patch.height {
            for x in patch.x..patch.x + patch.width {
                let at = (y * W + x) * 4;
                before[at..at + 4].copy_from_slice(&detail[at..at + 4]);
            }
        }
        let mut after = moved(&base, 3, 2, 0);
        for y in patch.y..patch.y + patch.height {
            for x in patch.x..patch.x + patch.width {
                let a = (y * W + x) * 4;
                let b = ((y - 5) * W + x + 3) * 4;
                after[b..b + 4].copy_from_slice(&detail[a..a + 4]);
            }
        }
        let a = snapshot(&before, &[]);
        let b = snapshot(&after, &[]);
        let foot = Region {
            x: 3,
            y: 30,
            width: 30,
            height: 66,
        };
        let ground = register_anchored(&a, &b, 20, foot).unwrap();
        let raised = register_local(&a, &b, 20, patch).unwrap();
        for point in [[18., 63.], [80., 65.]] {
            assert!(ground.supports(point));
            let p = ground.map_point(point);
            assert!((p[0] - point[0] - 3.).hypot(p[1] - point[1] - 2.) < 1.);
        }
        assert!((raised.map_point([80., 65.])[1] - 60.).abs() < 1.);
    }
}
