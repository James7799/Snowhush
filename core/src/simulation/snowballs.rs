//! Snowballs made by hand: scooped out of the drift on the ground, rolled along it to grow (taking the snow it rolls
//! over), carried with the pointer and let go. One let go over another lands on top of it; three stacked become a
//! snowman (`snowmen.rs`), its proportions those of the balls. A ball on the ground sits in the drift and rises with
//! it, as the snowmen do; a gust that clears the snow takes the balls too.
//! The app decides how a ball is grabbed: on X11, a ball itself takes the click, and Ctrl+drag scoops a new one.

use std::f32::consts::PI;

use super::{SnowSimulation, SnowballSprite};
use crate::geometry::V2;

/// A ball's radius when scooped, and the most it grows to by rolling, in points.
const START: f32 = 13.0;
const LARGEST: f32 = 46.0;
/// A scoop where little snow lies still makes a ball this big, to be rolled into a bigger one.
const SMALLEST: f32 = 8.0;
/// How far either side of the pointer a scoop gathers snow from.
const SCOOP_REACH: f32 = 40.0;
/// How deep a ball on the ground sits in the drift, as a share of its radius.
const SUNK: f32 = 0.35;
/// How much of the radius two stacked balls overlap by.
const STACK: f32 = 0.85;
const GRAVITY: f32 = 1500.0;
/// At most this many balls lie about at once.
const MOST: usize = 12;
/// How far above the drift the held ball may be and still roll along it, as a share of its radius: the drift sinks
/// under it as it takes the snow, and a hand dragging sideways drifts up and down.
const ROLL_REACH: f32 = 0.9;
/// How much snow rolling takes: square points per point rolled, per point of the ball's diameter.
const GATHER: f32 = 0.9;

pub(super) struct Ball {
    id: u64,
    center: V2,
    radius: f32,
    velocity: V2,
    /// How far it has turned as it rolled, in radians.
    roll: f32,
    /// Held by the pointer, this far from its centre.
    held: Option<V2>,
    resting: bool,
    /// The ball it rests on, if any.
    on: Option<u64>,
}

#[derive(Default)]
pub(super) struct Snowballs {
    balls: Vec<Ball>,
    next: u64,
    /// The pointer's latest place and motion while a ball is held, for the throw on letting go.
    pointer: Option<(V2, V2)>,
    pub(super) moved: bool,
}

impl Snowballs {
    pub(super) fn clear(&mut self) {
        self.moved |= !self.balls.is_empty();
        self.balls.clear();
        self.pointer = None;
    }
}

impl SnowSimulation {
    /// The ball under `point`, the front one where they overlap.
    pub fn snowball_at(&self, point: V2) -> Option<u64> {
        self.snowballs.balls.iter().rev().find(|b| (point - b.center).length() <= b.radius + 3.0).map(|b| b.id)
    }

    /// Scoops a ball out of the drift at `point`, held by the pointer; none where there is too little snow there
    /// or `point` is not down in the drift.
    pub fn scoop_snowball(&mut self, point: V2) -> Option<u64> {
        if self.snowballs.balls.len() >= MOST {
            return None;
        }
        let layer = &self.layers[0];
        let (from, to) = (layer.clamped_index(point.x - SCOOP_REACH), layer.clamped_index(point.x + SCOOP_REACH));
        let deepest = (from..=to).map(|i| layer.surface(i) - layer.floor(i)).fold(0.0, f32::max);
        let top = (from..=to).map(|i| layer.surface(i)).fold(0.0, f32::max);
        if point.y > top + 30.0 || deepest < 1.0 {
            return None;
        }
        let taken = self.take_drift_snow(point.x, SCOOP_REACH, PI * START * START);
        let radius = (taken / PI).sqrt().max(SMALLEST);
        let surface = self.drift_surface(point.x, radius);
        let id = self.snowballs.next;
        self.snowballs.next += 1;
        let center = V2::new(point.x, surface + radius * (1.0 - SUNK));
        self.snowballs.balls.push(Ball { id, center, radius, velocity: V2::ZERO, roll: 0.0, held: Some(center - point), resting: false, on: None });
        self.snowballs.pointer = Some((point, V2::ZERO));
        self.snowballs.moved = true;
        Some(id)
    }

    /// Takes hold of a ball; whatever was stacked on it falls.
    pub fn grab_snowball(&mut self, id: u64, point: V2) {
        for b in &mut self.snowballs.balls {
            if b.id == id {
                b.held = Some(b.center - point);
                b.resting = false;
                b.on = None;
            } else if b.on == Some(id) {
                b.resting = false;
                b.on = None;
            }
        }
        self.snowballs.pointer = Some((point, V2::ZERO));
        self.snowballs.moved = true;
    }

    /// Moves the held ball after the pointer. Along the drift it rolls, turning and growing with the snow it takes.
    pub fn drag_snowball(&mut self, point: V2, dt: f32) {
        let Some(index) = self.snowballs.balls.iter().position(|b| b.held.is_some()) else { return };
        if let Some((last, _)) = self.snowballs.pointer {
            self.snowballs.pointer = Some((point, (point - last) / dt.max(1.0 / 240.0)));
        }
        let (offset, radius, from) = {
            let b = &self.snowballs.balls[index];
            (b.held.unwrap_or(V2::ZERO), b.radius, b.center)
        };
        let mut to = point + offset;
        to.x = to.x.clamp(radius, (self.width - radius).max(radius));
        let ground = self.drift_surface(to.x, radius) + radius * (1.0 - SUNK);
        let rolling = to.y <= ground + (radius * ROLL_REACH).max(6.0);
        if rolling {
            to.y = ground;
        }
        let dx = to.x - from.x;
        let mut grown = radius;
        if rolling && dx.abs() > 0.5 && radius < LARGEST {
            let want = 2.0 * radius * dx.abs() * GATHER;
            let taken = self.take_drift_snow((from.x + to.x) / 2.0, radius + dx.abs() / 2.0, want);
            grown = (radius * radius + taken / PI).sqrt().min(LARGEST);
        }
        let b = &mut self.snowballs.balls[index];
        if rolling {
            b.roll -= dx / b.radius;
            to.y += (grown - b.radius) * (1.0 - SUNK);
        }
        b.radius = grown;
        b.center = to;
        self.snowballs.moved = true;
    }

    /// Lets go of the held ball, thrown on with the pointer's last motion, a little.
    pub fn release_snowball(&mut self) {
        let throw = self.snowballs.pointer.take().map_or(V2::ZERO, |(_, v)| v * 0.35);
        for b in &mut self.snowballs.balls {
            if b.held.take().is_some() {
                b.velocity = V2::new(throw.x.clamp(-600.0, 600.0), throw.y.clamp(-400.0, 600.0));
                b.resting = false;
                self.snowballs.moved = true;
            }
        }
    }

    pub fn snowballs(&self) -> Vec<SnowballSprite> {
        self.snowballs.balls.iter().map(|b| SnowballSprite { center: b.center, radius: b.radius, roll: b.roll }).collect()
    }

    /// Each ball's box, left, bottom, right, top: where it takes the pointer.
    pub fn snowball_boxes(&self) -> Vec<[f32; 4]> {
        self.snowballs
            .balls
            .iter()
            .map(|b| [b.center.x - b.radius, b.center.y - b.radius, b.center.x + b.radius, b.center.y + b.radius])
            .collect()
    }

    pub fn holding_snowball(&self) -> bool {
        self.snowballs.balls.iter().any(|b| b.held.is_some())
    }

    pub(super) fn snowballs_busy(&self) -> bool {
        self.snowballs.balls.iter().any(|b| b.held.is_some() || !b.resting)
    }

    pub(super) fn has_snowballs(&self) -> bool {
        !self.snowballs.balls.is_empty()
    }

    /// How high the drift on the ground reaches anywhere, in overlay coordinates: where scooping is possible.
    pub fn drift_top(&self) -> f32 {
        let layer = &self.layers[0];
        (0..layer.count()).map(|i| layer.surface(i)).fold(0.0, f32::max)
    }

    /// The drift's surface under a ball of this radius at `x`: the average across its middle.
    fn drift_surface(&self, x: f32, radius: f32) -> f32 {
        let layer = &self.layers[0];
        let (from, to) = (layer.clamped_index(x - radius * 0.5), layer.clamped_index(x + radius * 0.5));
        (from..=to).map(|i| layer.surface(i)).sum::<f32>() / (to - from + 1) as f32
    }

    /// Takes up to `area` square points of snow from the drift within `half` of `x`, evenly as deep as it is;
    /// returns how much it took.
    fn take_drift_snow(&mut self, x: f32, half: f32, area: f32) -> f32 {
        let layer = &mut self.layers[0];
        let width = layer.column_width;
        let (from, to) = (layer.clamped_index(x - half), layer.clamped_index(x + half));
        let depth: f32 = (from..=to).map(|i| (layer.surface(i) - layer.floor(i)).max(0.0)).sum();
        if depth <= 0.0 || area <= 0.0 {
            return 0.0;
        }
        let share = (area / width / depth).min(0.85);
        for i in from..=to {
            let h = (layer.surface(i) - layer.floor(i)).max(0.0);
            layer.remove_snow(h * share, i);
        }
        self.air.heights_changed = true;
        depth * share * width
    }

    /// Falling, landing in the drift or on another ball, and three stacked turning into a snowman.
    pub(super) fn tend_snowballs(&mut self, dt: f32) {
        if self.snowballs.balls.is_empty() {
            return;
        }
        let mut moved = false;
        let n = self.snowballs.balls.len();
        for k in 0..n {
            if self.snowballs.balls[k].held.is_some() {
                continue;
            }
            let (center, radius) = (self.snowballs.balls[k].center, self.snowballs.balls[k].radius);
            // What it would rest on: the drift, or a resting ball under it close enough to the middle.
            let support = self
                .snowballs
                .balls
                .iter()
                .filter(|o| o.id != self.snowballs.balls[k].id && o.held.is_none() && o.resting && o.center.y < center.y)
                .filter(|o| (o.center.x - center.x).abs() < o.radius * 0.7)
                .map(|o| (o.id, o.center.y + (o.radius + radius) * STACK, o.center.x))
                .max_by(|a, b| a.1.total_cmp(&b.1));
            let ground = self.drift_surface(center.x, radius) + radius * (1.0 - SUNK);
            let ball = &mut self.snowballs.balls[k];
            if ball.resting {
                match ball.on {
                    Some(under) => match support.filter(|s| s.0 == under) {
                        Some((_, y, x)) => {
                            ball.center = V2::new(x, y);
                        }
                        None => {
                            ball.resting = false;
                            ball.on = None;
                        }
                    },
                    None => {
                        // Riding up and down with the drift.
                        let next = ball.center.y + (ground - ball.center.y) * (dt * 5.0).min(1.0);
                        moved |= (next - ball.center.y).abs() > 0.01;
                        ball.center.y = next;
                    }
                }
                continue;
            }
            moved = true;
            ball.velocity.y -= GRAVITY * dt;
            ball.velocity.x *= (1.0 - 2.0 * dt).max(0.0);
            ball.center += ball.velocity * dt;
            ball.roll -= ball.velocity.x * dt / ball.radius;
            ball.center.x = ball.center.x.clamp(radius, (self.width - radius).max(radius));
            if let Some((id, y, x)) = support
                && ball.center.y <= y
            {
                ball.center = V2::new(x, y);
                ball.velocity = V2::ZERO;
                ball.resting = true;
                ball.on = Some(id);
            } else if ball.center.y <= ground {
                ball.center.y = ground;
                ball.velocity = V2::ZERO;
                ball.resting = true;
                ball.on = None;
            }
        }
        self.snowballs.moved |= moved;
        self.stack_snowmen();
    }

    /// Three resting balls stacked one on another make a snowman, the bottom one standing in the drift.
    fn stack_snowmen(&mut self) {
        let balls = &self.snowballs.balls;
        let find = |id: u64| balls.iter().find(|b| b.id == id);
        let mut built = None;
        for top in balls.iter().filter(|b| b.resting && b.held.is_none()) {
            let Some(middle) = top.on.and_then(find).filter(|b| b.resting) else { continue };
            let Some(bottom) = middle.on.and_then(find).filter(|b| b.resting && b.on.is_none()) else { continue };
            built = Some((bottom.id, middle.id, top.id, bottom.center, bottom.radius, middle.radius / bottom.radius, top.radius / bottom.radius));
            break;
        }
        let Some((a, b, c, center, radius, middle, head)) = built else { return };
        self.snowballs.balls.retain(|ball| ![a, b, c].contains(&ball.id));
        self.snowballs.moved = true;
        self.stand_snowman(center.x, center.y - radius * 0.92, radius, [1.0, middle, head]);
    }
}

#[cfg(test)]
mod tests {
    use crate::geometry::V2;
    use crate::simulation::SnowSimulation;

    const DT: f32 = 1.0 / 60.0;
    const AWAY: V2 = V2::new(-10_000.0, -10_000.0);

    fn run(simulation: &mut SnowSimulation, seconds: f32) {
        for _ in 0..(seconds / DT) as usize {
            simulation.step(DT, AWAY);
        }
    }

    /// Carries the held ball to `point` and holds it still there a frame, so letting go doesn't throw it.
    fn carry(simulation: &mut SnowSimulation, point: V2) {
        simulation.drag_snowball(point, DT);
        simulation.drag_snowball(point, DT);
    }

    fn depth(simulation: &SnowSimulation, x: f32) -> f32 {
        let layer = &simulation.layers[0];
        let i = layer.clamped_index(x);
        layer.surface(i) - layer.floor(i)
    }

    #[test]
    fn balls_are_scooped_rolled_stacked_and_become_a_snowman() {
        let mut simulation = SnowSimulation::new(1200.0, 800.0, 0.0);
        for i in 0..simulation.layers[0].count() {
            simulation.layers[0].set(i, 40.0);
        }
        assert!(simulation.scoop_snowball(V2::new(600.0, 400.0)).is_none(), "nothing to scoop up in the air");
        let before = depth(&simulation, 300.0);
        let a = simulation.scoop_snowball(V2::new(300.0, 30.0)).expect("a ball");
        assert!(depth(&simulation, 300.0) < before, "the drift gave its snow");
        // Rolled along the drift, it grows.
        let small = simulation.snowballs()[0].radius;
        for k in 1..=60 {
            simulation.drag_snowball(V2::new(300.0 + k as f32 * 5.0, 30.0), DT);
        }
        carry(&mut simulation, V2::new(600.0, 30.0));
        let big = simulation.snowballs()[0].radius;
        assert!(big > small * 1.5, "{small} -> {big}");
        simulation.release_snowball();
        run(&mut simulation, 1.0);
        let bottom = simulation.snowballs()[0];
        assert!(simulation.snowball_at(bottom.center) == Some(a));

        // A smaller one let go over it lands on top.
        simulation.scoop_snowball(V2::new(900.0, 30.0)).expect("a second ball");
        carry(&mut simulation, V2::new(bottom.center.x, bottom.center.y + 200.0));
        simulation.release_snowball();
        run(&mut simulation, 1.0);
        let middle = simulation.snowballs()[1];
        assert!((middle.center.x - bottom.center.x).abs() < 0.5 && middle.center.y > bottom.center.y + bottom.radius);
        assert!(simulation.snowmen().is_empty());

        simulation.scoop_snowball(V2::new(100.0, 30.0)).expect("a head");
        carry(&mut simulation, V2::new(bottom.center.x, middle.center.y + 250.0));
        simulation.release_snowball();
        run(&mut simulation, 1.5);
        assert!(simulation.snowballs().is_empty(), "the three became a snowman");
        let snowman = simulation.snowmen()[0];
        assert!((snowman.anchor.x - bottom.center.x).abs() < 0.5 && (snowman.size - bottom.radius).abs() < 0.5);
        assert!(snowman.radii[1] < 1.0 && snowman.radii[2] < snowman.radii[1] + 0.5);
    }

    #[test]
    fn a_thin_drift_still_gives_a_small_ball() {
        let mut simulation = SnowSimulation::new(1200.0, 800.0, 0.0);
        assert!(simulation.scoop_snowball(V2::new(300.0, 2.0)).is_none(), "no snow, no ball");
        for i in 0..simulation.layers[0].count() {
            simulation.layers[0].set(i, 3.0);
        }
        simulation.scoop_snowball(V2::new(300.0, 2.0)).expect("a ball from thin snow");
        assert!(simulation.snowballs()[0].radius >= 8.0);
    }

    #[test]
    fn a_ball_rolled_along_a_shallow_drift_grows_though_the_hand_is_a_little_high() {
        let mut simulation = SnowSimulation::new(1200.0, 800.0, 0.0);
        for i in 0..simulation.layers[0].count() {
            simulation.layers[0].set(i, 12.0);
        }
        simulation.scoop_snowball(V2::new(200.0, 8.0)).expect("a ball");
        let small = simulation.snowballs()[0].radius;
        for k in 1..=100 {
            simulation.drag_snowball(V2::new(200.0 + k as f32 * 6.0, 8.0 + (k % 3) as f32 * 3.0), DT);
        }
        let big = simulation.snowballs()[0].radius;
        assert!(big > small * 1.4, "{small} -> {big}");
    }

    #[test]
    fn grabbing_a_ball_drops_what_lies_on_it_and_a_gust_takes_them() {
        let mut simulation = SnowSimulation::new(1200.0, 800.0, 0.0);
        for i in 0..simulation.layers[0].count() {
            simulation.layers[0].set(i, 40.0);
        }
        let a = simulation.scoop_snowball(V2::new(500.0, 30.0)).expect("a ball");
        simulation.release_snowball();
        run(&mut simulation, 0.5);
        let under = simulation.snowballs()[0].center;
        simulation.scoop_snowball(V2::new(800.0, 30.0)).expect("another");
        carry(&mut simulation, V2::new(under.x, under.y + 150.0));
        simulation.release_snowball();
        run(&mut simulation, 1.0);
        let on_top = simulation.snowballs()[1].center;
        assert!(on_top.y > under.y);
        simulation.grab_snowball(a, under);
        simulation.drag_snowball(V2::new(200.0, 300.0), DT);
        run(&mut simulation, 1.0);
        assert!(simulation.snowballs()[1].center.y < on_top.y - 5.0, "it fell");
        simulation.release_snowball();
        simulation.blow_away(1.0, false);
        run(&mut simulation, 0.1);
        assert!(simulation.snowballs().is_empty());
    }
}
