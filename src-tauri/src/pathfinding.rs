//! Bounded world-map planning. Geometry comes from a supplied/observed map.
//! Distances are in the map's horizontal coordinate plane, not 3D geodesics.
//! Convex miter offsets conservatively contain circular footprint inflation.
use serde::{Deserialize, Serialize};

pub type Point = [f64; 2];
const EPS: f64 = 1e-8;
const MAX_COORD: f64 = 1_000_000.0;
const MAX_SURFACES: usize = 8;
const MAX_OBSTACLES: usize = 32;
const MAX_VERTICES: usize = 256;
const MAX_JUMPS: usize = 32;
const MAX_NODES: usize = 512;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Map {
    pub surfaces: Vec<Surface>,
    pub obstacles: Vec<Obstacle>,
    pub jumps: Vec<Jump>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Surface {
    pub id: u32,
    /// Simple convex polygon; either winding is accepted. No holes.
    pub boundary: Vec<Point>,
    pub height: Plane,
    /// Absolute overhead clearance, where present over this surface.
    pub ceiling: Option<f64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Plane {
    /// World height = base + gradient[0] * x + gradient[1] * z.
    pub base: f64,
    pub gradient: Point,
}

impl Plane {
    fn at(self, p: Point) -> f64 {
        self.base + dot(self.gradient, p)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Obstacle {
    pub boundary: Vec<Point>,
    /// Solid vertical prism. A bridge floor/ceiling must be represented explicitly.
    pub bottom: f64,
    pub top: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Jump {
    pub id: u32,
    pub from_surface: u32,
    pub to_surface: u32,
    pub takeoff: Point,
    pub landing: Point,
    /// Additional rise over the linear takeoff-to-landing height at t=0.5.
    /// The supplied, measured arc is h(t)=lerp(h0,h1,t)+4*apex*t*(1-t).
    pub apex: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Agent {
    pub radius: f64,
    pub safety: f64,
    pub height: f64,
    pub max_slope_degrees: f64,
    pub max_jump_distance: f64,
    /// Maximum arc height above takeoff, not only the landing height difference.
    pub max_jump_up: f64,
    pub max_jump_down: f64,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Position {
    pub surface: u32,
    pub point: Point,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Goal {
    pub surface: u32,
    pub center: Point,
    #[serde(default)]
    pub radius: f64,
    /// Optional convex region for permitted avatar centers, e.g. coastal land.
    pub allowed: Option<Vec<Point>>,
    /// Ground-line visibility to an interaction center is required by default.
    /// Coastal goals may opt out only with an explicit permitted shore region.
    #[serde(default = "default_true")]
    pub require_center_sight: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub start: Position,
    pub goal: Goal,
    pub agent: Agent,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Segment {
    Walk {
        surface: u32,
        from: Point,
        to: Point,
    },
    Jump {
        id: u32,
        from: Position,
        to: Position,
        apex: f64,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Route {
    /// Explicit metric: shortest horizontal map-plane length on modeled geometry.
    pub distance_metric: String,
    pub direct_distance: f64,
    pub route_distance: f64,
    /// Geometric arrival is not proof of task completion (e.g. fishing available).
    pub already_arrived: bool,
    pub end: Position,
    pub segments: Vec<Segment>,
}

struct PreparedSurface {
    id: u32,
    boundary: Vec<Point>,
    height: Plane,
    ceiling: Option<f64>,
    blocked: Vec<Vec<Point>>,
}

struct PreparedObstacle {
    boundary: Vec<Point>,
    bottom: f64,
    top: f64,
}

/// Visibility-graph Dijkstra, including a goal region and directed jump edges.
/// Exact for the prepared planar polygon model and its horizontal-distance metric;
/// conservative offsets/observed map accuracy are not a claim of exact 3D optimality.
pub fn plan(map: &Map, request: &Request) -> Result<Route, String> {
    validate_request(request)?;
    let (surfaces, obstacles) = prepare(map, request.agent)?;
    let start_surface = surface(&surfaces, request.start.surface)?;
    if !position_valid(start_surface, request.start.point) {
        return Err("Start is outside traversable clearance.".into());
    }
    let goal_surface = surface(&surfaces, request.goal.surface)?;
    if request.goal.require_center_sight && !position_valid(goal_surface, request.goal.center) {
        return Err("Interaction goal center is not on the traversable target surface.".into());
    }
    let mut goal_region = goal_surface.boundary.clone();
    if let Some(allowed) = &request.goal.allowed {
        let allowed = polygon(allowed)?;
        goal_region = intersect(&goal_region, &allowed);
    }
    if goal_region.len() < 3 {
        return Err("Goal region has no traversable surface.".into());
    }
    let goal_edges = goal_edges(goal_surface, &goal_region, &request.goal);
    let mut nodes = vec![request.start];
    for s in &surfaces {
        for polygon in std::iter::once(&s.boundary).chain(s.blocked.iter()) {
            for &point in polygon {
                if position_valid(s, point) {
                    add_node(
                        &mut nodes,
                        Position {
                            surface: s.id,
                            point,
                        },
                    )?;
                }
            }
        }
    }
    let mut jump_edges = Vec::new();
    for jump in &map.jumps {
        let from = surface(&surfaces, jump.from_surface)?;
        let to = surface(&surfaces, jump.to_surface)?;
        if jump_valid(jump, from, to, &surfaces, &obstacles, request.agent) {
            let a = add_node(
                &mut nodes,
                Position {
                    surface: from.id,
                    point: jump.takeoff,
                },
            )?;
            let b = add_node(
                &mut nodes,
                Position {
                    surface: to.id,
                    point: jump.landing,
                },
            )?;
            jump_edges.push((a, b, jump));
        }
    }
    let count = nodes.len();
    let mut distances = vec![f64::INFINITY; count];
    let mut visited = vec![false; count];
    let mut previous: Vec<Option<(usize, Segment)>> = vec![None; count];
    distances[0] = 0.0;
    let mut best = f64::INFINITY;
    let mut terminal = None;
    for _ in 0..count {
        let Some(u) = (0..count)
            .filter(|&i| !visited[i])
            .min_by(|&a, &b| distances[a].total_cmp(&distances[b]))
        else {
            break;
        };
        if !distances[u].is_finite() || distances[u] >= best {
            break;
        }
        visited[u] = true;
        let from = nodes[u];
        if from.surface == request.goal.surface {
            if let Some(point) = goal_connection(
                from.point,
                goal_surface,
                &goal_region,
                &goal_edges,
                &request.goal,
            ) {
                let cost = distances[u] + distance(from.point, point);
                if cost < best {
                    best = cost;
                    terminal = Some((u, point));
                }
            }
        }
        let s = surface(&surfaces, from.surface)?;
        for v in 0..count {
            if visited[v] || nodes[v].surface != from.surface {
                continue;
            }
            let cost = distances[u] + distance(from.point, nodes[v].point);
            if cost + EPS < distances[v] && walk_clear(s, from.point, nodes[v].point) {
                distances[v] = cost;
                previous[v] = Some((
                    u,
                    Segment::Walk {
                        surface: s.id,
                        from: from.point,
                        to: nodes[v].point,
                    },
                ));
            }
        }
        for &(a, b, jump) in &jump_edges {
            if a != u || visited[b] {
                continue;
            }
            let cost = distances[u] + distance(nodes[a].point, nodes[b].point);
            if cost + EPS < distances[b] {
                distances[b] = cost;
                previous[b] = Some((
                    u,
                    Segment::Jump {
                        id: jump.id,
                        from: nodes[a],
                        to: nodes[b],
                        apex: jump.apex,
                    },
                ));
            }
        }
    }
    let (mut cursor, end) =
        terminal.ok_or("Goal region is unreachable with validated walk/jump edges.")?;
    let last = nodes[cursor];
    let mut segments = Vec::new();
    while cursor != 0 {
        let (parent, segment) = previous[cursor]
            .clone()
            .ok_or("Invalid route predecessor.")?;
        segments.push(segment);
        cursor = parent;
    }
    segments.reverse();
    if distance(last.point, end) > EPS {
        segments.push(Segment::Walk {
            surface: last.surface,
            from: last.point,
            to: end,
        });
    }
    Ok(Route {
        distance_metric: "mapPlane".into(),
        direct_distance: distance(request.start.point, request.goal.center),
        route_distance: best,
        already_arrived: segments.is_empty(),
        end: Position {
            surface: request.goal.surface,
            point: end,
        },
        segments,
    })
}

fn validate_request(request: &Request) -> Result<(), String> {
    let a = request.agent;
    if !valid_point(request.start.point)
        || !valid_point(request.goal.center)
        || !finite_nonnegative(request.goal.radius)
        || request.goal.radius > MAX_COORD
        || ![
            a.radius,
            a.safety,
            a.max_jump_distance,
            a.max_jump_up,
            a.max_jump_down,
        ]
        .into_iter()
        .all(|value| finite_nonnegative(value) && value <= 10_000.0)
        || !a.height.is_finite()
        || !(0.01..=10_000.0).contains(&a.height)
        || !a.max_slope_degrees.is_finite()
        || !(0.0..90.0).contains(&a.max_slope_degrees)
    {
        return Err("Invalid position, goal, or agent limits.".into());
    }
    if let Some(allowed) = &request.goal.allowed {
        polygon(allowed)?;
    } else if !request.goal.require_center_sight {
        return Err("A goal without center sight requires an explicit permitted region.".into());
    }
    Ok(())
}

fn prepare(
    map: &Map,
    agent: Agent,
) -> Result<(Vec<PreparedSurface>, Vec<PreparedObstacle>), String> {
    if map.surfaces.is_empty()
        || map.surfaces.len() > MAX_SURFACES
        || map.obstacles.len() > MAX_OBSTACLES
        || map.jumps.len() > MAX_JUMPS
        || map
            .surfaces
            .iter()
            .map(|s| s.boundary.len())
            .chain(map.obstacles.iter().map(|o| o.boundary.len()))
            .sum::<usize>()
            > MAX_VERTICES
    {
        return Err("Map exceeds geometry limits.".into());
    }
    let clearance = agent.radius + agent.safety;
    let mut obstacles = Vec::new();
    for o in &map.obstacles {
        if !valid_scalar(o.bottom) || !valid_scalar(o.top) || o.bottom >= o.top {
            return Err("Invalid obstacle height interval.".into());
        }
        let boundary =
            offset(&polygon(&o.boundary)?, -clearance).ok_or("Invalid obstacle offset.")?;
        obstacles.push(PreparedObstacle {
            boundary,
            bottom: o.bottom,
            top: o.top,
        });
    }
    let mut surfaces = Vec::new();
    for s in &map.surfaces {
        if surfaces.iter().any(|p: &PreparedSurface| p.id == s.id)
            || !valid_scalar(s.height.base)
            || !s.height.gradient.into_iter().all(valid_scalar)
            || s.ceiling.is_some_and(|value| !valid_scalar(value))
        {
            return Err("Invalid surface id, height, or ceiling.".into());
        }
        let source = polygon(&s.boundary)?;
        let mut boundary = offset(&source, clearance).unwrap_or_default();
        if dot(s.height.gradient, s.height.gradient)
            .sqrt()
            .atan()
            .to_degrees()
            > agent.max_slope_degrees + EPS
        {
            boundary.clear();
        }
        if let Some(ceiling) = s.ceiling {
            boundary = clip_height(
                &boundary,
                s.height,
                f64::NEG_INFINITY,
                ceiling - agent.height - agent.safety,
            );
        }
        let mut blocked = Vec::new();
        for o in &obstacles {
            // Footprint safety is horizontal. A designated walkable surface can
            // support the feet at a prism's top; touching it is not penetration.
            let p = if dot(s.height.gradient, s.height.gradient) <= EPS * EPS
                && (s.height.base >= o.top - EPS || s.height.base + agent.height <= o.bottom + EPS)
            {
                Vec::new()
            } else {
                clip_height(&o.boundary, s.height, o.bottom - agent.height, o.top)
            };
            // Keep the original obstruction past the world boundary. Clipping it
            // to that boundary would create a fictitious edge one could walk on.
            if p.len() >= 3 && area(&p).abs() > EPS {
                blocked.push(p);
            }
        }
        surfaces.push(PreparedSurface {
            id: s.id,
            boundary,
            height: s.height,
            ceiling: s.ceiling,
            blocked,
        });
    }
    for (i, jump) in map.jumps.iter().enumerate() {
        if map.jumps[..i].iter().any(|j| j.id == jump.id)
            || !valid_point(jump.takeoff)
            || !valid_point(jump.landing)
            || !finite_nonnegative(jump.apex)
            || jump.apex > 10_000.0
        {
            return Err("Invalid jump id, endpoints, or measured arc.".into());
        }
        surface(&surfaces, jump.from_surface)?;
        surface(&surfaces, jump.to_surface)?;
    }
    Ok((surfaces, obstacles))
}

fn surface(surfaces: &[PreparedSurface], id: u32) -> Result<&PreparedSurface, String> {
    surfaces
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| format!("Unknown surface {id}."))
}

fn add_node(nodes: &mut Vec<Position>, value: Position) -> Result<usize, String> {
    if let Some(index) = nodes
        .iter()
        .position(|p| p.surface == value.surface && distance(p.point, value.point) <= EPS)
    {
        return Ok(index);
    }
    if nodes.len() >= MAX_NODES {
        return Err("Map exceeds visibility graph node limit.".into());
    }
    nodes.push(value);
    Ok(nodes.len() - 1)
}

fn position_valid(s: &PreparedSurface, p: Point) -> bool {
    inside(&s.boundary, p, false) && !s.blocked.iter().any(|o| inside(o, p, true))
}

fn walk_clear(s: &PreparedSurface, a: Point, b: Point) -> bool {
    position_valid(s, a)
        && position_valid(s, b)
        && !s
            .blocked
            .iter()
            .any(|p| segment_interval(p, a, b, true).is_some())
}

fn jump_valid(
    j: &Jump,
    from: &PreparedSurface,
    to: &PreparedSurface,
    surfaces: &[PreparedSurface],
    obstacles: &[PreparedObstacle],
    agent: Agent,
) -> bool {
    if !position_valid(from, j.takeoff)
        || !position_valid(to, j.landing)
        || distance(j.takeoff, j.landing) > agent.max_jump_distance + EPS
    {
        return false;
    }
    let h0 = from.height.at(j.takeoff);
    let h1 = to.height.at(j.landing);
    let (_, peak) = arc_range(h0, h1, j.apex, 0.0, 1.0);
    if peak - h0 > agent.max_jump_up + EPS || h0 - h1 > agent.max_jump_down + EPS {
        return false;
    }
    for o in obstacles {
        if let Some((lo, hi)) = segment_interval(&o.boundary, j.takeoff, j.landing, true) {
            let (low, high) = arc_range(h0, h1, j.apex, lo, hi);
            if low < o.top + agent.safety - EPS
                && high + agent.height > o.bottom - agent.safety + EPS
            {
                return false;
            }
        }
    }
    for s in surfaces {
        // A lower layer's room ceiling is not an infinite solid above it.
        // Other physical slabs crossed in the air belong in the obstacle map.
        if s.id != from.id && s.id != to.id {
            continue;
        }
        if let Some(ceiling) = s.ceiling {
            if let Some((lo, hi)) = segment_interval(&s.boundary, j.takeoff, j.landing, false) {
                if arc_range(h0, h1, j.apex, lo, hi).1 + agent.height + agent.safety > ceiling + EPS
                {
                    return false;
                }
            }
        }
    }
    true
}

fn arc_range(h0: f64, h1: f64, apex: f64, lo: f64, hi: f64) -> (f64, f64) {
    let height = |t: f64| h0 + (h1 - h0) * t + 4.0 * apex * t * (1.0 - t);
    let (a, b) = (height(lo), height(hi));
    let peak_t = if apex > EPS {
        ((h1 - h0 + 4.0 * apex) / (8.0 * apex)).clamp(lo, hi)
    } else {
        lo
    };
    (a.min(b), a.max(b).max(height(peak_t)))
}

fn goal_connection(
    from: Point,
    s: &PreparedSurface,
    region: &[Point],
    edges: &[(Point, Point)],
    goal: &Goal,
) -> Option<Point> {
    let mut best = None;
    let mut best_distance = f64::INFINITY;
    let mut consider = |p: Point| {
        let length = distance(from, p);
        if length < best_distance
            && distance(p, goal.center) <= goal.radius + EPS
            && inside(region, p, false)
            && walk_clear(s, from, p)
            && (!goal.require_center_sight || walk_clear(s, p, goal.center))
        {
            best = Some(p);
            best_distance = length;
        }
    };
    consider(from);
    consider(goal.center);
    let toward = sub(from, goal.center);
    let length = dot(toward, toward).sqrt();
    if length > EPS {
        consider(add(goal.center, mul(toward, goal.radius / length)));
    }
    // Also consider obstacle boundaries: a goal disk can overlap an obstacle.
    // Merely projecting to the disk or routing to its center misses these entries.
    for &(a, b) in edges {
        let Some((lo, hi)) = segment_interval(region, a, b, false) else {
            continue;
        };
        let Some((lo, hi)) = disk_interval(a, b, goal.center, goal.radius, lo, hi) else {
            continue;
        };
        let direction = sub(b, a);
        let denominator = dot(direction, direction);
        if denominator <= EPS * EPS {
            continue;
        }
        let t = (dot(sub(from, a), direction) / denominator).clamp(lo, hi);
        consider(add(a, mul(direction, t)));
    }
    best
}

fn goal_edges(s: &PreparedSurface, region: &[Point], goal: &Goal) -> Vec<(Point, Point)> {
    let mut edges = Vec::new();
    for polygon in std::iter::once(region).chain(s.blocked.iter().map(Vec::as_slice)) {
        for i in 0..polygon.len() {
            edges.push((polygon[i], polygon[(i + 1) % polygon.len()]));
        }
    }
    if goal.require_center_sight && goal.radius > EPS {
        // Visibility-region boundaries are rays through obstacle corners.
        // Include them so arrival need not go all the way to a corner/center.
        for &corner in s.blocked.iter().flatten() {
            let direction = sub(corner, goal.center);
            let length = dot(direction, direction).sqrt();
            if length > EPS {
                edges.push((
                    goal.center,
                    add(goal.center, mul(direction, goal.radius / length)),
                ));
            }
        }
    }
    edges
}

fn disk_interval(
    a: Point,
    b: Point,
    center: Point,
    radius: f64,
    lo: f64,
    hi: f64,
) -> Option<(f64, f64)> {
    let d = sub(b, a);
    let q = sub(a, center);
    let aa = dot(d, d);
    if aa <= EPS * EPS {
        return (dot(q, q) <= radius * radius + EPS).then_some((lo, hi));
    }
    let bb = 2.0 * dot(q, d);
    let cc = dot(q, q) - radius * radius;
    let discriminant = bb * bb - 4.0 * aa * cc;
    if discriminant < -EPS {
        return None;
    }
    let root = discriminant.max(0.0).sqrt();
    let left = lo.max((-bb - root) / (2.0 * aa));
    let right = hi.min((-bb + root) / (2.0 * aa));
    (left <= right + EPS).then_some((left.clamp(lo, hi), right.clamp(lo, hi)))
}

fn polygon(points: &[Point]) -> Result<Vec<Point>, String> {
    if !(3..=32).contains(&points.len()) || !points.iter().copied().all(valid_point) {
        return Err("Expected a finite convex polygon with 3..32 vertices.".into());
    }
    let mut p = points.to_vec();
    for i in 0..p.len() {
        if p[..i].iter().any(|&v| distance(v, p[i]) <= EPS) {
            return Err("Polygon must not repeat vertices.".into());
        }
    }
    if area(&p) < 0.0 {
        p.reverse();
    }
    if area(&p) <= EPS {
        return Err("Polygon has no area.".into());
    }
    for i in 0..p.len() {
        let edge = sub(p[(i + 1) % p.len()], p[i]);
        if dot(edge, edge) <= EPS * EPS
            || p.iter()
                .any(|&v| cross(edge, sub(v, p[i])) < -EPS * dot(edge, edge).sqrt())
        {
            return Err("Polygon must be simple and convex without duplicate vertices.".into());
        }
    }
    loop {
        let redundant = (0..p.len()).find(|&i| {
            cross(
                sub(p[i], p[(i + p.len() - 1) % p.len()]),
                sub(p[(i + 1) % p.len()], p[i]),
            )
            .abs()
                <= EPS
        });
        match redundant {
            Some(i) if p.len() > 3 => {
                p.remove(i);
            }
            _ => break,
        }
    }
    Ok(p)
}

fn offset(p: &[Point], inward: f64) -> Option<Vec<Point>> {
    let mut lines = Vec::new();
    for i in 0..p.len() {
        let edge = sub(p[(i + 1) % p.len()], p[i]);
        let norm = dot(edge, edge).sqrt();
        let shift = mul([-edge[1], edge[0]], inward / norm);
        lines.push((add(p[i], shift), edge));
    }
    let mut result = Vec::new();
    for i in 0..lines.len() {
        let (a, da) = lines[(i + lines.len() - 1) % lines.len()];
        let (b, db) = lines[i];
        let denominator = cross(da, db);
        if denominator.abs() <= EPS {
            return None;
        }
        let point = add(a, mul(da, cross(sub(b, a), db) / denominator));
        if !valid_point(point)
            || lines
                .iter()
                .any(|&(a, d)| cross(d, sub(point, a)) < -EPS * dot(d, d).sqrt())
        {
            return None;
        }
        result.push(point);
    }
    (area(&result) > EPS).then_some(result)
}

fn intersect(subject: &[Point], clip: &[Point]) -> Vec<Point> {
    if subject.len() < 3 || clip.len() < 3 {
        return Vec::new();
    }
    let mut result = subject.to_vec();
    for i in 0..clip.len() {
        let a = clip[i];
        let edge = sub(clip[(i + 1) % clip.len()], a);
        result = clip_linear(&result, [-edge[1], edge[0]], dot([-edge[1], edge[0]], a));
    }
    result
}

fn clip_height(p: &[Point], plane: Plane, low: f64, high: f64) -> Vec<Point> {
    let mut result = p.to_vec();
    if low.is_finite() {
        result = clip_linear(&result, plane.gradient, low - plane.base);
    }
    if high.is_finite() {
        result = clip_linear(&result, mul(plane.gradient, -1.0), plane.base - high);
    }
    result
}

/// Keep dot(normal,p)>=minimum. This also handles a constant-height plane.
fn clip_linear(p: &[Point], normal: Point, minimum: f64) -> Vec<Point> {
    let mut result = Vec::new();
    for i in 0..p.len() {
        let (a, b) = (p[i], p[(i + 1) % p.len()]);
        let (da, db) = (dot(normal, a) - minimum, dot(normal, b) - minimum);
        if da >= -EPS {
            push_unique(&mut result, a);
        }
        if (da >= 0.0) != (db >= 0.0) {
            push_unique(&mut result, add(a, mul(sub(b, a), da / (da - db))));
        }
    }
    if result.len() > 1 && distance(result[0], *result.last().unwrap()) <= EPS {
        result.pop();
    }
    result
}

fn push_unique(points: &mut Vec<Point>, p: Point) {
    if points.last().is_none_or(|last| distance(*last, p) > EPS) {
        points.push(p);
    }
}

fn inside(p: &[Point], point: Point, strict: bool) -> bool {
    p.len() >= 3
        && (0..p.len()).all(|i| {
            let edge = sub(p[(i + 1) % p.len()], p[i]);
            let d = cross(edge, sub(point, p[i]));
            if strict {
                d > EPS * dot(edge, edge).sqrt()
            } else {
                d >= -EPS * dot(edge, edge).sqrt()
            }
        })
}

fn segment_interval(p: &[Point], a: Point, b: Point, strict: bool) -> Option<(f64, f64)> {
    if p.len() < 3 {
        return None;
    }
    let (mut lo, mut hi) = (0.0_f64, 1.0_f64);
    for i in 0..p.len() {
        let edge = sub(p[(i + 1) % p.len()], p[i]);
        let tolerance = EPS * dot(edge, edge).sqrt();
        let c = cross(edge, sub(a, p[i]));
        let d = cross(edge, sub(b, a));
        let minimum = if strict { tolerance } else { -tolerance };
        if d.abs() <= tolerance {
            if c < minimum {
                return None;
            }
        } else {
            let t = (minimum - c) / d;
            if d > 0.0 {
                lo = lo.max(t);
            } else {
                hi = hi.min(t);
            }
        }
        if lo > hi {
            return None;
        }
    }
    if strict && hi - lo <= EPS {
        None
    } else {
        Some((lo, hi))
    }
}

fn area(p: &[Point]) -> f64 {
    (0..p.len())
        .map(|i| cross(p[i], p[(i + 1) % p.len()]))
        .sum::<f64>()
        / 2.0
}
fn valid_scalar(value: f64) -> bool {
    value.is_finite() && value.abs() <= MAX_COORD
}
fn valid_point(p: Point) -> bool {
    p.into_iter().all(valid_scalar)
}
fn finite_nonnegative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}
fn add(a: Point, b: Point) -> Point {
    [a[0] + b[0], a[1] + b[1]]
}
fn sub(a: Point, b: Point) -> Point {
    [a[0] - b[0], a[1] - b[1]]
}
fn mul(p: Point, value: f64) -> Point {
    [p[0] * value, p[1] * value]
}
fn dot(a: Point, b: Point) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}
fn cross(a: Point, b: Point) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}
fn distance(a: Point, b: Point) -> f64 {
    let d = sub(a, b);
    dot(d, d).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]]
    }
    fn surface_at(id: u32, height: f64) -> Surface {
        Surface {
            id,
            boundary: rectangle(-5.0, -5.0, 20.0, 10.0),
            height: Plane {
                base: height,
                gradient: [0.0, 0.0],
            },
            ceiling: None,
        }
    }
    fn map() -> Map {
        Map {
            surfaces: vec![surface_at(1, 0.0)],
            obstacles: vec![],
            jumps: vec![],
        }
    }
    fn request() -> Request {
        Request {
            start: Position {
                surface: 1,
                point: [0.0, 0.0],
            },
            goal: Goal {
                surface: 1,
                center: [10.0, 0.0],
                radius: 0.0,
                allowed: None,
                require_center_sight: true,
            },
            agent: Agent {
                radius: 0.0,
                safety: 0.0,
                height: 1.8,
                max_slope_degrees: 40.0,
                max_jump_distance: 8.0,
                max_jump_up: 4.0,
                max_jump_down: 4.0,
            },
        }
    }
    fn wall(boundary: Vec<Point>, top: f64) -> Obstacle {
        Obstacle {
            boundary,
            bottom: -1.0,
            top,
        }
    }
    fn near(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    #[test]
    fn straight_route_stops_at_goal_circle_and_distinguishes_distances() {
        let mut r = request();
        r.goal.radius = 2.0;
        let path = plan(&map(), &r).unwrap();
        near(path.direct_distance, 10.0);
        near(path.route_distance, 8.0);
        assert_eq!(path.end.point, [8.0, 0.0]);
        assert_eq!(path.segments.len(), 1);
        r.start.point = [9.0, 0.0];
        assert!(plan(&map(), &r).unwrap().already_arrived);
    }
    #[test]
    fn obstacle_route_is_the_taut_shortest_corner_path() {
        let mut m = map();
        m.obstacles.push(wall(rectangle(4.0, -1.0, 6.0, 1.0), 3.0));
        let path = plan(&m, &request()).unwrap();
        near(path.route_distance, 2.0 * 17.0_f64.sqrt() + 2.0);
        assert_eq!(path.segments.len(), 3);
        for segment in path.segments {
            if let Segment::Walk { from, to, .. } = segment {
                assert!(walk_clear(
                    &prepare(&m, request().agent).unwrap().0[0],
                    from,
                    to
                ));
            } else {
                panic!("unexpected jump");
            }
        }
    }
    #[test]
    fn diagonal_corner_keeps_full_footprint_clearance() {
        let mut m = map();
        m.obstacles.push(wall(rectangle(4.0, -1.0, 6.0, 1.0), 3.0));
        let mut r = request();
        r.agent.radius = 0.5;
        r.agent.safety = 0.25;
        let path = plan(&m, &r).unwrap();
        near(
            path.route_distance,
            2.0 * (3.25_f64.powi(2) + 1.75_f64.powi(2)).sqrt() + 3.5,
        );
        assert!(path
            .segments
            .iter()
            .any(|s| matches!(s,Segment::Walk{to,..} if (to[1].abs()-1.75).abs()<EPS)));
    }
    #[test]
    fn goal_circle_overlapping_obstacle_uses_reachable_region_boundary() {
        let mut m = map();
        m.obstacles.push(wall(rectangle(8.0, -2.0, 12.0, 2.0), 3.0));
        let mut r = request();
        r.goal.radius = 3.0;
        r.goal.require_center_sight = false;
        r.goal.allowed = Some(rectangle(-4.0, -4.0, 19.0, 9.0));
        let path = plan(&m, &r).unwrap();
        near(path.route_distance, 7.0);
        near(path.end.point[0], 7.0);
        r.goal.radius = 1.0;
        assert!(plan(&m, &r).is_err());
    }
    #[test]
    fn same_coordinates_on_wrong_layer_are_not_arrival() {
        let mut m = map();
        m.surfaces.push(surface_at(2, 3.0));
        let mut r = request();
        r.goal.surface = 2;
        r.goal.center = r.start.point;
        r.goal.radius = 1.0;
        assert!(plan(&m, &r).is_err());
    }
    #[test]
    fn directed_jump_is_planned_before_wall_and_preserved() {
        let mut m = map();
        m.obstacles.push(wall(rectangle(4.0, -6.0, 6.0, 11.0), 1.0));
        m.jumps.push(Jump {
            id: 7,
            from_surface: 1,
            to_surface: 1,
            takeoff: [3.0, 0.0],
            landing: [7.0, 0.0],
            apex: 2.0,
        });
        let path = plan(&m, &request()).unwrap();
        assert_eq!(path.segments.len(), 3);
        assert!(matches!(path.segments[1], Segment::Jump { id: 7, .. }));
        assert!(matches!(
            path.segments[0],
            Segment::Walk { to: [3.0, 0.0], .. }
        ));
        let mut reverse = request();
        reverse.start.point = [10.0, 0.0];
        reverse.goal.center = [0.0, 0.0];
        assert!(plan(&m, &reverse).is_err());
        m.jumps[0].apex = 0.2;
        assert!(plan(&m, &request()).is_err());
        m.jumps[0].apex = 2.0;
        let mut short = request();
        short.agent.max_jump_distance = 3.0;
        assert!(plan(&m, &short).is_err());
    }
    #[test]
    fn slope_height_and_headroom_are_enforced() {
        let mut m = map();
        m.surfaces[0].height.gradient = [1.0, 0.0];
        assert!(plan(&m, &request()).is_err());
        m.surfaces[0].height.gradient = [0.0, 0.0];
        m.surfaces[0].ceiling = Some(1.0);
        assert!(plan(&m, &request()).is_err());
        m.surfaces[0].ceiling = None;
        m.surfaces.push(surface_at(2, 5.0));
        m.jumps.push(Jump {
            id: 1,
            from_surface: 1,
            to_surface: 2,
            takeoff: [1.0, 0.0],
            landing: [2.0, 0.0],
            apex: 1.0,
        });
        let mut r = request();
        r.goal.surface = 2;
        assert!(plan(&m, &r).is_err());
    }
    #[test]
    fn coastal_allowed_region_is_used_instead_of_fixed_fishing_point() {
        let mut r = request();
        r.goal.center = [10.0, -4.0];
        r.goal.radius = 5.0;
        r.goal.allowed = Some(rectangle(8.0, -2.0, 12.0, -1.0));
        r.goal.require_center_sight = false;
        let path = plan(&map(), &r).unwrap();
        assert!(inside(
            r.goal.allowed.as_ref().unwrap(),
            path.end.point,
            false
        ));
        assert!(distance(path.end.point, r.goal.center) <= r.goal.radius + EPS);
        near(path.end.point[0], 8.0);
        near(path.end.point[1], -1.0);
    }
    #[test]
    fn interaction_arrival_requires_center_sight_and_handles_tangent_entry() {
        let mut m = map();
        m.obstacles.push(wall(rectangle(4.0, -1.0, 6.0, 1.0), 3.0));
        let mut r = request();
        r.goal.radius = 10.0;
        let path = plan(&m, &r).unwrap();
        assert!(!path.already_arrived);
        near(path.route_distance, 10.0 / 17.0_f64.sqrt());
        m.obstacles[0].boundary = rectangle(1.0, -6.0, 1.5, 11.0);
        r.goal.center = [2.0, 0.0];
        r.goal.radius = 3.0;
        assert!(plan(&m, &r).is_err());
        let goal: Goal =
            serde_json::from_value(serde_json::json!({"surface":1,"center":[2,0],"allowed":null}))
                .unwrap();
        assert!(goal.require_center_sight);
        assert_eq!(goal.radius, 0.0);
    }
    #[test]
    fn clipped_coastal_disk_can_end_on_walkable_boundary() {
        let mut m = map();
        m.surfaces[0].boundary = rectangle(0.0, 0.0, 10.0, 10.0);
        let mut r = request();
        r.start.point = [9.0, 1.0];
        r.goal.center = [11.0, 11.0];
        r.goal.radius = 2.0;
        r.goal.allowed = Some(rectangle(0.0, 0.0, 10.0, 10.0));
        r.goal.require_center_sight = false;
        let path = plan(&m, &r).unwrap();
        near(path.end.point[0], 10.0);
        near(path.end.point[1], 11.0 - 3.0_f64.sqrt());
    }
    #[test]
    fn supported_prism_top_and_unrelated_lower_ceiling_do_not_block_upper_route() {
        let mut m = map();
        m.obstacles.push(wall(rectangle(4.0, -1.0, 6.0, 1.0), 0.0));
        let mut lower = surface_at(2, -3.0);
        lower.ceiling = Some(-1.0);
        m.surfaces.push(lower);
        let route = plan(&m, &request()).unwrap();
        near(route.route_distance, 10.0);
        m.obstacles[0].boundary = rectangle(4.0, -6.0, 6.0, 11.0);
        m.obstacles[0].top = 1.0;
        m.jumps.push(Jump {
            id: 7,
            from_surface: 1,
            to_surface: 1,
            takeoff: [3.0, 0.0],
            landing: [7.0, 0.0],
            apex: 2.0,
        });
        assert!(plan(&m, &request())
            .unwrap()
            .segments
            .iter()
            .any(|s| matches!(s, Segment::Jump { .. })));
    }
    #[test]
    fn rejects_nonconvex_nan_oversize_and_unreachable_maps() {
        let mut r = request();
        r.start.point[0] = f64::NAN;
        assert!(plan(&map(), &r).is_err());
        let mut m = map();
        m.surfaces[0].boundary = vec![[0.0, 0.0], [10.0, 0.0], [5.0, 2.0], [10.0, 5.0], [0.0, 5.0]];
        assert!(plan(&m, &request()).is_err());
        m = map();
        m.surfaces = vec![surface_at(1, 0.0); MAX_SURFACES + 1];
        assert!(plan(&m, &request()).is_err());
        m = map();
        m.obstacles.push(wall(rectangle(4.0, -6.0, 6.0, 11.0), 3.0));
        assert!(plan(&m, &request()).is_err());
        let mut r = request();
        r.agent.radius = 100.0;
        assert!(plan(&map(), &r).is_err());
    }
    #[test]
    fn clockwise_geometry_and_serialized_contract_roundtrip() {
        let mut m = map();
        m.surfaces[0].boundary.reverse();
        let m: Map = serde_json::from_str(&serde_json::to_string(&m).unwrap()).unwrap();
        let route = plan(&m, &request()).unwrap();
        let encoded = serde_json::to_value(route).unwrap();
        assert_eq!(encoded["distanceMetric"], "mapPlane");
        assert_eq!(encoded["segments"][0]["kind"], "walk");
    }
}
