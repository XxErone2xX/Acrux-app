//! Persistent, observed overhead terrain. Each chart has only image-derived
//! coordinates; disconnected screenshots never acquire invented world positions.
use super::{distance, inside, Contact, Destination, Grid};
use crate::{
    scene::{self, Snapshot},
    vision::{Frame, Point, Rect},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{Read, Write},
};

const SIDE: usize = 384;
const CELL: usize = 4;
const MAX_TILES: usize = 65_536;
const MAX_FRAMES: usize = 24;
const MAX_COORD: f64 = 32_768.;
type Pose = [[f64; 3]; 2];
const IDENTITY: Pose = [[1., 0., 0.], [0., 1., 0.]];

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Atlas {
    pub places: Vec<PlaceMemory>,
    frames: Vec<Keyframe>,
    tiles: Vec<Tile>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PlaceMemory {
    pub destination: Destination,
    pub point: [f64; 2],
    chart: u32,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Keyframe {
    chart: u32,
    width: usize,
    height: usize,
    gray: String,
    excluded: Vec<Rect>,
    pose: Pose,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Tile {
    chart: u32,
    x: i32,
    y: i32,
    floor: bool,
    visits: u16,
}
struct View {
    width: usize,
    height: usize,
    gray: Vec<u8>,
    excluded: Vec<Rect>,
    pose: Pose,
    chart: u32,
}
pub(super) struct MapGoal {
    pub point: Point,
    /// True only when this is the actual place, rather than a visible waypoint.
    pub arrival: bool,
}
pub(super) struct Mapper {
    frames: Vec<Keyframe>,
    places: Vec<PlaceMemory>,
    tiles: BTreeMap<(u32, i32, i32), Tile>,
    current: Option<View>,
    recent: VecDeque<(u32, [f64; 2])>,
}
fn apply(a: Pose, p: [f64; 2]) -> [f64; 2] {
    a.map(|r| r[0] * p[0] + r[1] * p[1] + r[2])
}
fn compose(a: Pose, b: Pose) -> Pose {
    a.map(|r| {
        [
            r[0] * b[0][0] + r[1] * b[1][0],
            r[0] * b[0][1] + r[1] * b[1][1],
            r[0] * b[0][2] + r[1] * b[1][2] + r[2],
        ]
    })
}
fn inverse(a: Pose) -> Option<Pose> {
    let d = a[0][0] * a[1][1] - a[0][1] * a[1][0];
    if !d.is_finite() || d <= 0.01 {
        return None;
    }
    let mut out = [
        [a[1][1] / d, -a[0][1] / d, 0.],
        [-a[1][0] / d, a[0][0] / d, 0.],
    ];
    for r in &mut out {
        r[2] = -r[0] * a[0][2] - r[1] * a[1][2];
    }
    Some(out)
}
fn valid_pose(a: Pose) -> bool {
    a.iter()
        .flatten()
        .all(|v| v.is_finite() && v.abs() <= MAX_COORD)
        && inverse(a).is_some()
        && (0..2).all(|i| (0.15..=6.).contains(&a[0][i].hypot(a[1][i])))
}
fn valid_point(p: [f64; 2]) -> bool {
    p.into_iter().all(|v| v.is_finite() && v.abs() <= MAX_COORD)
}
fn valid_rect(r: Rect) -> bool {
    [r.0, r.1, r.2, r.3]
        .into_iter()
        .all(|v| v.is_finite() && (0.0..=1.0).contains(&v))
        && r.0 < r.2
        && r.1 < r.3
}
fn decode(k: &Keyframe) -> Result<View, String> {
    if k.width < 24
        || k.height < 24
        || k.width > SIDE
        || k.height > SIDE
        || k.gray.len() > SIDE * SIDE * 2
        || k.excluded.len() > 24
        || !k.excluded.iter().copied().all(valid_rect)
        || !valid_pose(k.pose)
    {
        return Err("저장된 지형 관측 데이터의 범위가 올바르지 않습니다.".into());
    }
    let encoded = STANDARD
        .decode(&k.gray)
        .map_err(|_| "저장된 지형 이미지가 올바르지 않습니다.")?;
    let len = k.width * k.height;
    let mut gray = Vec::with_capacity(len);
    ZlibDecoder::new(encoded.as_slice())
        .take(len as u64 + 1)
        .read_to_end(&mut gray)
        .map_err(|_| "저장된 지형 이미지를 읽지 못했습니다.")?;
    if gray.len() != len {
        return Err("저장된 지형 이미지 크기가 올바르지 않습니다.".into());
    }
    Ok(View {
        width: k.width,
        height: k.height,
        gray,
        excluded: k.excluded.clone(),
        pose: k.pose,
        chart: k.chart,
    })
}
impl View {
    fn center(&self) -> [f64; 2] {
        apply(
            self.pose,
            [self.width as f64 * 0.5, self.height as f64 * 0.5],
        )
    }
    fn local(&self, p: [f64; 2]) -> Option<Point> {
        let p = apply(inverse(self.pose)?, p);
        Some(Point(p[0] / self.width as f64, p[1] / self.height as f64))
    }
    fn keyframe(&self) -> Result<Keyframe, String> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
        encoder
            .write_all(&self.gray)
            .map_err(|_| "지형 이미지 저장에 실패했습니다.")?;
        let compressed = encoder
            .finish()
            .map_err(|_| "지형 이미지 저장에 실패했습니다.")?;
        Ok(Keyframe {
            chart: self.chart,
            width: self.width,
            height: self.height,
            gray: STANDARD.encode(compressed),
            excluded: self.excluded.clone(),
            pose: self.pose,
        })
    }
    fn capture(frame: &Frame<'_>, excluded: &[Rect]) -> Result<Self, String> {
        if !frame.is_full() || frame.width < 24 || frame.height < 24 || excluded.len() > 23 {
            return Err("지형 지도에는 새 전체 게임 화면이 필요합니다.".into());
        }
        let scale = SIDE as f64 / frame.width.max(frame.height) as f64;
        let (width, height) = (
            (frame.width as f64 * scale).round() as usize,
            (frame.height as f64 * scale).round() as usize,
        );
        if width < 24 || height < 24 {
            return Err("게임 화면의 비율이 지형 관측 범위를 벗어났습니다.".into());
        }
        let mut gray = vec![0; width * height];
        for y in 0..height {
            let top = y as f64 * frame.height as f64 / height as f64;
            let bottom = (y + 1) as f64 * frame.height as f64 / height as f64;
            for x in 0..width {
                let left = x as f64 * frame.width as f64 / width as f64;
                let right = (x + 1) as f64 * frame.width as f64 / width as f64;
                let mut sum = 0.;
                // Integrate the actual source pixel area. Nearest sampling
                // aliases a fractional camera pan into changing floor texture.
                for sy in top.floor() as usize..(bottom.ceil() as usize).min(frame.height) {
                    let wy = bottom.min(sy as f64 + 1.) - top.max(sy as f64);
                    for sx in left.floor() as usize..(right.ceil() as usize).min(frame.width) {
                        let wx = right.min(sx as f64 + 1.) - left.max(sx as f64);
                        let [r, g, b] = frame.pixel(sx, sy).ok_or("지형 화면 픽셀이 없습니다.")?;
                        sum += (77 * u32::from(r) + 150 * u32::from(g) + 29 * u32::from(b)) as f64
                            * wx
                            * wy;
                    }
                }
                gray[y * width + x] = (sum / ((right - left) * (bottom - top)) / 256.)
                    .round()
                    .clamp(0., 255.) as u8;
            }
        }
        let mut excluded: Vec<_> = excluded
            .iter()
            .copied()
            .filter(|r| valid_rect(*r))
            .collect();
        excluded.push(Rect(0.455, 0.425, 0.545, 0.575));
        Ok(Self {
            width,
            height,
            gray,
            excluded,
            pose: IDENTITY,
            chart: 0,
        })
    }
}
/// Both snapshots use exactly the same union of masks. A HUD or avatar visible
/// in only one image is never allowed to become a stationary map landmark.
fn align(reference: &View, current: &View, radius: usize) -> Option<Pose> {
    if reference.width != current.width || reference.height != current.height {
        return None;
    }
    let geometry = (current.width, current.height);
    let included = Rect(0.12, 0.08, 0.98, 0.82).region(geometry)?;
    let mut masks: Vec<_> = reference
        .excluded
        .iter()
        .chain(&current.excluded)
        .filter_map(|r| r.region(geometry))
        .collect();
    masks.sort_by_key(|r| (r.x, r.y, r.width, r.height));
    masks.dedup();
    if masks.len() > 32 {
        return None;
    }
    let rgb = |v: &View| v.gray.iter().flat_map(|&n| [n, n, n]).collect::<Vec<_>>();
    let before = rgb(reference);
    let after = rgb(current);
    let a = Frame::new(&before, geometry.0, geometry.1, geometry.0 * 3, 3, false)?;
    let b = Frame::new(&after, geometry.0, geometry.1, geometry.0 * 3, 3, false)?;
    let a = Snapshot::capture(&a, included, &masks).ok()?;
    let b = Snapshot::capture(&b, included, &masks).ok()?;
    let registration = scene::register(&a, &b, radius).ok()?;
    if registration.error_pixels > 5.
        || registration.scale.iter().any(|s| !(0.65..=1.5).contains(s))
        || registration.rotation_radians.abs() > 0.4
    {
        return None;
    }
    // The affine must be witnessed on both sides of the camera subject.
    let source_center = apply(
        inverse(registration.affine)?,
        [geometry.0 as f64 * 0.5, geometry.1 as f64 * 0.5],
    );
    if !registration.supports(source_center) {
        return None;
    }
    let pose = compose(reference.pose, inverse(registration.affine)?);
    valid_pose(pose).then_some(pose)
}
impl Mapper {
    pub(super) fn new(atlas: Option<Atlas>) -> Result<Self, String> {
        let atlas = atlas.unwrap_or_default();
        if atlas.frames.len() > MAX_FRAMES
            || atlas.tiles.len() > MAX_TILES
            || atlas.places.len() > 5
        {
            return Err("저장된 지형 지도의 크기가 제한을 넘었습니다.".into());
        }
        let mut charts = Vec::new();
        for k in &atlas.frames {
            decode(k)?;
            if !charts.contains(&k.chart) {
                charts.push(k.chart);
            }
        }
        let mut tiles = BTreeMap::new();
        for tile in atlas.tiles {
            if !charts.contains(&tile.chart)
                || tile.x.unsigned_abs() > MAX_COORD as u32 / CELL as u32
                || tile.y.unsigned_abs() > MAX_COORD as u32 / CELL as u32
            {
                return Err("저장된 지형 지도의 좌표가 올바르지 않습니다.".into());
            }
            let key = (tile.chart, tile.x, tile.y);
            if tiles.insert(key, tile).is_some() {
                return Err("저장된 지형 지도에 중복 좌표가 있습니다.".into());
            }
        }
        let mut destinations = Vec::new();
        for place in &atlas.places {
            if !valid_point(place.point)
                || !charts.contains(&place.chart)
                || destinations.contains(&place.destination)
            {
                return Err("저장된 장소 좌표가 올바르지 않습니다.".into());
            }
            destinations.push(place.destination);
        }
        Ok(Self {
            frames: atlas.frames,
            places: atlas.places,
            tiles,
            current: None,
            recent: VecDeque::new(),
        })
    }
    pub(super) fn memory(&self) -> Option<Atlas> {
        (!self.frames.is_empty()).then(|| Atlas {
            places: self.places.clone(),
            frames: self.frames.clone(),
            tiles: self.tiles.values().cloned().collect(),
        })
    }
    pub(super) fn observe(
        &mut self,
        frame: &Frame<'_>,
        grid: &mut Grid,
        excluded: &[Rect],
    ) -> Result<(), String> {
        let mut view = View::capture(frame, excluded)?;
        let mut located = self
            .current
            .as_ref()
            .and_then(|reference| align(reference, &view, 80).map(|pose| (reference.chart, pose)));
        if located.is_none() {
            for k in self.frames.iter().rev() {
                let reference = decode(k)?;
                if let Some(pose) = align(&reference, &view, 192) {
                    located = Some((k.chart, pose));
                    break;
                }
            }
        }
        if let Some((chart, pose)) = located {
            view.chart = chart;
            view.pose = pose;
        } else {
            view.chart = self
                .frames
                .iter()
                .map(|k| k.chart)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .ok_or("지형 지도 번호 범위를 넘었습니다.")?;
        }
        let center = view.center();
        if !valid_point(center) {
            return Err("관측 지형 좌표 범위를 넘었습니다.".into());
        }
        let add_frame = !self.frames.iter().any(|k| {
            k.chart == view.chart
                && distance(
                    apply(k.pose, [k.width as f64 * 0.5, k.height as f64 * 0.5]),
                    center,
                ) < 36.
        });
        if add_frame {
            if self.frames.len() >= MAX_FRAMES {
                // Keep an image for every saved destination chart. If a chart
                // loses its final keyframe its unmatched occupancy is discarded.
                let remove = self
                    .frames
                    .iter()
                    .position(|k| {
                        !self.places.iter().any(|p| p.chart == k.chart)
                            || self.frames.iter().filter(|f| f.chart == k.chart).count() > 1
                    })
                    .unwrap_or(0);
                let old = self.frames.remove(remove).chart;
                if !self.frames.iter().any(|k| k.chart == old) {
                    self.tiles.retain(|(chart, _, _), _| *chart != old);
                    self.places.retain(|p| p.chart != old);
                }
            }
            self.frames.push(view.keyframe()?);
        }
        self.restore_occlusion(&view, grid, excluded);
        self.accumulate(&view, grid);
        if self
            .recent
            .back()
            .is_none_or(|(chart, p)| *chart != view.chart || distance(*p, center) > 8.)
        {
            self.recent.push_back((view.chart, center));
            if self.recent.len() > 48 {
                self.recent.pop_front();
            }
        }
        self.current = Some(view);
        Ok(())
    }
    fn restore_occlusion(&self, view: &View, grid: &mut Grid, excluded: &[Rect]) {
        // The old chart is read BEFORE accumulating this frame. Only an
        // independently registered, previously witnessed floor tile may fill
        // the camera subject's mask; visible obstacles and UI stay untouched.
        for at in 0..grid.land.len() {
            let p = grid.normalized(grid.pixel(at));
            if grid.visible[at]
                || grid.land[at]
                || !inside(grid.body_mask, p)
                || excluded.iter().any(|&r| inside(r, p))
            {
                continue;
            }
            let world = apply(
                view.pose,
                [p.0 * view.width as f64, p.1 * view.height as f64],
            );
            let key = (
                view.chart,
                (world[0] / CELL as f64).floor() as i32,
                (world[1] / CELL as f64).floor() as i32,
            );
            if self.tiles.get(&key).is_some_and(|tile| tile.floor) {
                grid.land[at] = true;
                grid.plane[at] = true;
            }
        }
    }
    fn accumulate(&mut self, view: &View, grid: &Grid) {
        let corners = [
            [0., 0.],
            [view.width as f64, 0.],
            [0., view.height as f64],
            [view.width as f64, view.height as f64],
        ]
        .map(|p| apply(view.pose, p));
        let minimum = |axis: usize| {
            corners
                .iter()
                .map(|p| p[axis])
                .fold(f64::INFINITY, f64::min)
                .floor() as i32
                / CELL as i32
        };
        let maximum = |axis: usize| {
            corners
                .iter()
                .map(|p| p[axis])
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil() as i32
                / CELL as i32
        };
        let (x0, y0, x1, y1) = (minimum(0), minimum(1), maximum(0), maximum(1));
        if x1 - x0 > 1024
            || y1 - y0 > 1024
            || i64::from(x1 - x0) * i64::from(y1 - y0) > MAX_TILES as i64
        {
            return;
        }
        let center = view.center();
        let foot_radius =
            (grid.step as f64 * 2. / grid.geometry.0 as f64 * view.width as f64).max(4.);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let point = [
                    (x as f64 + 0.5) * CELL as f64,
                    (y as f64 + 0.5) * CELL as f64,
                ];
                if !valid_point(point) {
                    continue;
                }
                let Some(local) = view.local(point) else {
                    continue;
                };
                if !(0.0..1.0).contains(&local.0) || !(0.0..1.0).contains(&local.1) {
                    continue;
                }
                let at = grid.index(local);
                let footprint = (distance(point, center) <= foot_radius
                    || inside(grid.body_mask, local))
                    && grid.land[at];
                if !grid.visible[at] && !footprint {
                    continue;
                }
                let key = (view.chart, x, y);
                if self.tiles.len() >= MAX_TILES && !self.tiles.contains_key(&key) {
                    continue;
                }
                let tile = self.tiles.entry(key).or_insert(Tile {
                    chart: view.chart,
                    x,
                    y,
                    floor: grid.land[at],
                    visits: 0,
                });
                tile.floor = grid.land[at];
                if distance(point, center) < 12. {
                    tile.visits = tile.visits.saturating_add(1);
                }
            }
        }
    }
    pub(super) fn mark(&mut self, destination: Destination) {
        let Some(view) = &self.current else {
            return;
        };
        let place = PlaceMemory {
            destination,
            point: view.center(),
            chart: view.chart,
        };
        self.places.retain(|p| p.destination != destination);
        self.places.push(place);
    }
    pub(super) fn remember_visible(&mut self, destination: Destination, point: Point) {
        let Some(view) = &self.current else {
            return;
        };
        if !super::valid(point) {
            return;
        }
        let point = apply(
            view.pose,
            [point.0 * view.width as f64, point.1 * view.height as f64],
        );
        if !valid_point(point) {
            return;
        }
        self.places.retain(|p| p.destination != destination);
        self.places.push(PlaceMemory {
            destination,
            point,
            chart: view.chart,
        });
    }
    fn global_grid(&self, chart: u32) -> Option<(Grid, [f64; 2])> {
        let tiles: Vec<_> = self.tiles.values().filter(|t| t.chart == chart).collect();
        let x0 = tiles.iter().map(|t| t.x).min()?;
        let x1 = tiles.iter().map(|t| t.x).max()?;
        let y0 = tiles.iter().map(|t| t.y).min()?;
        let y1 = tiles.iter().map(|t| t.y).max()?;
        let (w, h) = ((x1 - x0 + 1) as usize, (y1 - y0 + 1) as usize);
        if w > 1024 || h > 1024 || w.checked_mul(h)? > MAX_TILES {
            return None;
        }
        let mut land = vec![false; w * h];
        for tile in tiles {
            land[(tile.y - y0) as usize * w + (tile.x - x0) as usize] = tile.floor;
        }
        let n = w * h;
        Some((
            Grid {
                w,
                h,
                step: CELL,
                geometry: (w * CELL, h * CELL),
                land,
                open: vec![false; n],
                visible: vec![true; n],
                plane: vec![false; n],
                plausible: vec![false; n],
                colors: vec![[0; 3]; n],
                floor_color: [0; 3],
                body_mask: Rect(0., 0., 0., 0.),
                start: 0,
            },
            [x0 as f64 * CELL as f64, y0 as f64 * CELL as f64],
        ))
    }
    pub(super) fn next(
        &self,
        destination: Destination,
        grid: &Grid,
        contact: Contact,
    ) -> Option<MapGoal> {
        let view = self.current.as_ref()?;
        let place = self
            .places
            .iter()
            .find(|p| p.destination == destination && p.chart == view.chart)?;
        let local = view.local(place.point)?;
        let clearance = contact.radius + contact.error + 3.;
        if super::valid(local) && grid.clear(grid.index(local), clearance) {
            return Some(MapGoal {
                point: local,
                arrival: true,
            });
        }
        let (global, origin) = self.global_grid(view.chart)?;
        let center = view.center();
        let local_map = |p: [f64; 2]| global.normalized([p[0] - origin[0], p[1] - origin[1]]);
        let scale = view.width as f64 / grid.geometry.0 as f64;
        let pose_scale = view.pose[0][0]
            .hypot(view.pose[1][0])
            .max(view.pose[0][1].hypot(view.pose[1][1]));
        let radius = (clearance * scale * pose_scale).max(CELL as f64);
        let route = global.route(local_map(center), local_map(place.point), radius)?;
        // Keep the global corner direction, but issue a waypoint only inside
        // currently observed, locally reachable ground. Unknown pixels stay shut.
        for segment in route.windows(2).rev() {
            for t in (1..=24).rev() {
                let t = t as f64 / 24.;
                let q = [
                    segment[0][0] + (segment[1][0] - segment[0][0]) * t + origin[0],
                    segment[0][1] + (segment[1][1] - segment[0][1]) * t + origin[1],
                ];
                let Some(p) = view.local(q) else {
                    continue;
                };
                if !inside(Rect(0.16, 0.12, 0.94, 0.78), p)
                    || distance([p.0, p.1], [contact.point.0, contact.point.1]) < 0.05
                {
                    continue;
                }
                if grid.clear(grid.index(p), clearance)
                    && grid.route(contact.point, p, clearance).is_some()
                {
                    return Some(MapGoal {
                        point: p,
                        arrival: false,
                    });
                }
            }
        }
        None
    }
    pub(super) fn frontier(&mut self, grid: &Grid, contact: Contact) -> Option<Point> {
        let view = self.current.as_ref()?;
        let clearance = contact.radius + contact.error + 3.;
        let start = grid.index(contact.point);
        let mut seen = vec![false; grid.land.len()];
        let mut queue = VecDeque::from([start]);
        seen[start] = true;
        let mut candidates = Vec::new();
        while let Some(at) = queue.pop_front() {
            let p = grid.normalized(grid.pixel(at));
            if inside(Rect(0.17, 0.13, 0.93, 0.77), p) && grid.clear(at, clearance) {
                let world = apply(
                    view.pose,
                    [p.0 * view.width as f64, p.1 * view.height as f64],
                );
                let key = (
                    view.chart,
                    (world[0] / CELL as f64).floor() as i32,
                    (world[1] / CELL as f64).floor() as i32,
                );
                let unknown = [(-3, 0), (3, 0), (0, -3), (0, 3)]
                    .into_iter()
                    .filter(|(dx, dy)| !self.tiles.contains_key(&(key.0, key.1 + dx, key.2 + dy)))
                    .count();
                let edge = (p.0 - 0.5).hypot(p.1 - 0.5);
                let travel = distance(world, view.center());
                if travel > 24. {
                    let recent = self
                        .recent
                        .iter()
                        .filter(|(chart, _)| *chart == view.chart)
                        .map(|(_, q)| (-distance(world, *q) / 24.).exp())
                        .sum::<f64>();
                    let visits = f64::from(self.tiles.get(&key).map_or(0, |t| t.visits));
                    candidates.push((
                        unknown as f64 * 12. + edge * 20. + travel * 0.03
                            - recent * 15.
                            - visits * 4.,
                        p,
                        world,
                    ));
                }
            }
            let (x, y) = (at % grid.w, at / grid.w);
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let (Some(x), Some(y)) = (x.checked_add_signed(dx), y.checked_add_signed(dy))
                else {
                    continue;
                };
                if x < grid.w && y < grid.h {
                    let next = y * grid.w + x;
                    if !seen[next] && grid.clear(next, clearance) {
                        seen[next] = true;
                        queue.push_back(next);
                    }
                }
            }
        }
        candidates.sort_by(|a, b| b.0.total_cmp(&a.0));
        let (_, point, world) = candidates
            .into_iter()
            .find(|(_, p, _)| grid.route(contact.point, *p, clearance).is_some())?;
        let key = (
            view.chart,
            (world[0] / CELL as f64).floor() as i32,
            (world[1] / CELL as f64).floor() as i32,
        );
        if let Some(tile) = self.tiles.get_mut(&key) {
            tile.visits = tile.visits.saturating_add(4);
        }
        Some(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world(w: usize, h: usize) -> Vec<u8> {
        let mut out = vec![0; w * h * 3];
        let mut seed = 7u32;
        for y in 0..h {
            for x in 0..w {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let n = ((seed >> 24) % 19) as u8;
                let at = (y * w + x) * 3;
                out[at..at + 3].copy_from_slice(&[72 + n, 142 + n, 62 + n]);
            }
        }
        out
    }
    fn view(world: &[u8], world_w: usize, x: usize) -> Vec<u8> {
        let mut out = vec![0; 384 * 216 * 3];
        for y in 0..216 {
            out[y * 384 * 3..(y + 1) * 384 * 3]
                .copy_from_slice(&world[(y * world_w + x) * 3..(y * world_w + x + 384) * 3]);
        }
        out
    }
    fn observed(bytes: &[u8]) -> (Frame<'_>, Grid, Contact) {
        let frame = Frame::new(bytes, 384, 216, 384 * 3, 3, false).unwrap();
        let contact = super::super::camera_contact(&frame).unwrap();
        let grid = Grid::observe(&frame, contact, &[]).unwrap();
        (frame, grid, contact)
    }
    #[test]
    fn centered_place_relocalizes_from_ground_after_camera_pan_without_names() {
        let world = world(900, 216);
        let mut mapper = Mapper::new(None).unwrap();
        let a = view(&world, 900, 0);
        let (frame, mut grid, _) = observed(&a);
        mapper.observe(&frame, &mut grid, &[]).unwrap();
        mapper.mark(Destination::Base);
        let chart = mapper.places[0].chart;
        let b = view(&world, 900, 60);
        let (frame, mut grid, contact) = observed(&b);
        mapper.observe(&frame, &mut grid, &[]).unwrap();
        assert_eq!(mapper.current.as_ref().unwrap().chart, chart);
        let target = mapper.next(Destination::Base, &grid, contact).unwrap();
        assert!(target.arrival);
        assert!((target.point.0 - (192. - 60.) / 384.).abs() < 0.02);
        let saved = mapper.memory().unwrap();
        let mut restored = Mapper::new(Some(saved)).unwrap();
        restored.observe(&frame, &mut grid, &[]).unwrap();
        assert_eq!(restored.current.as_ref().unwrap().chart, chart);
    }
    #[test]
    fn fractional_camera_pan_at_real_viewport_resolution_preserves_the_chart() {
        const W: usize = 800;
        const H: usize = 270;
        let mut world = vec![0; W * H * 3];
        for y in 0..H {
            for x in 0..W {
                let mut seed = (x / 4) as u32 ^ ((y / 4) as u32).wrapping_mul(0x9e3779b9);
                seed ^= seed >> 16;
                seed = seed.wrapping_mul(0x7feb352d);
                seed ^= seed >> 15;
                let n = (seed % 25) as u8;
                world[(y * W + x) * 3..(y * W + x) * 3 + 3].copy_from_slice(&[
                    72 + n,
                    142 + n,
                    62 + n,
                ]);
            }
        }
        let mut mapper = Mapper::new(None).unwrap();
        let mut chart = 0;
        for offset in [0, 8, 16] {
            let mut pixels = vec![0; 480 * H * 3];
            for y in 0..H {
                pixels[y * 480 * 3..(y + 1) * 480 * 3]
                    .copy_from_slice(&world[(y * W + offset) * 3..(y * W + offset + 480) * 3]);
            }
            let frame = Frame::new(&pixels, 480, H, 480 * 3, 3, false).unwrap();
            let contact = super::super::camera_contact(&frame).unwrap();
            let mut grid = Grid::observe(&frame, contact, &[]).unwrap();
            mapper.observe(&frame, &mut grid, &[]).unwrap();
            if offset == 0 {
                mapper.mark(Destination::Base);
                chart = mapper.current.as_ref().unwrap().chart;
            } else {
                assert_eq!(
                    mapper.current.as_ref().unwrap().chart,
                    chart,
                    "a fractional thumbnail shift must not create a disconnected chart"
                );
                let goal = mapper.next(Destination::Base, &grid, contact).unwrap();
                assert!((goal.point.0 - (240. - offset as f64) / 480.).abs() < 0.025);
            }
        }
    }
    #[test]
    fn offscreen_saved_place_uses_accumulated_observed_ground_waypoint() {
        let world = world(1200, 216);
        let mut mapper = Mapper::new(None).unwrap();
        for x in (0..=280).step_by(40) {
            let pixels = view(&world, 1200, x);
            let (frame, mut grid, _) = observed(&pixels);
            mapper.observe(&frame, &mut grid, &[]).unwrap();
            if x == 0 {
                mapper.mark(Destination::Sell);
            }
        }
        let pixels = view(&world, 1200, 280);
        let (frame, mut grid, contact) = observed(&pixels);
        mapper.observe(&frame, &mut grid, &[]).unwrap();
        let goal = mapper
            .next(Destination::Sell, &grid, contact)
            .expect("known terrain must lead toward an offscreen saved location");
        assert!(!goal.arrival);
        assert!(goal.point.0 < 0.45);
        assert!(grid
            .route(contact.point, goal.point, contact.radius + contact.error)
            .is_some());
    }
    #[test]
    fn atlas_rejects_invalid_coordinates_and_frontier_never_crosses_unknown_ground() {
        let mut atlas = Atlas::default();
        atlas.places.push(PlaceMemory {
            destination: Destination::Craft,
            point: [f64::NAN, 0.],
            chart: 1,
        });
        assert!(Mapper::new(Some(atlas)).is_err());
        let world = world(384, 216);
        let (frame, mut grid, contact) = observed(&world);
        let mut mapper = Mapper::new(None).unwrap();
        mapper.observe(&frame, &mut grid, &[]).unwrap();
        let mut malformed = mapper.memory().unwrap();
        malformed.tiles.push(Tile {
            chart: mapper.current.as_ref().unwrap().chart,
            x: i32::MIN,
            y: 0,
            floor: true,
            visits: 0,
        });
        assert!(Mapper::new(Some(malformed)).is_err());
        let view = mapper.current.as_ref().unwrap();
        let blocked = grid.start;
        grid.visible[blocked] = true;
        grid.land[blocked] = false;
        grid.plane[blocked] = false;
        mapper.restore_occlusion(view, &mut grid, &[]);
        assert!(
            !grid.land[blocked],
            "registered floor must never erase a currently visible obstacle"
        );
        grid.visible[blocked] = false;
        let ui = Rect(0.49, 0.49, 0.53, 0.54);
        mapper.restore_occlusion(view, &mut grid, &[ui]);
        assert!(
            !grid.land[blocked],
            "registered floor must not restore pixels hidden by a UI exclusion"
        );
        grid.land[blocked] = true;
        for y in 0..grid.h {
            for x in grid.w * 3 / 5..grid.w {
                grid.land[y * grid.w + x] = false;
            }
        }
        let point = mapper.frontier(&grid, contact).unwrap();
        assert!(point.0 < 0.6);
        assert!(grid
            .route(contact.point, point, contact.radius + contact.error)
            .is_some());
    }
}
