//! Snowmen: built from the menu (three balls of snow, the bottom one rolled in along the drift and the other two
//! dropped onto it, then coal eyes, a carrot nose, buttons, a scarf, twig arms and a hat), or stacked by hand from
//! snowballs (`snowballs.rs`). Each stands in the drift the way Halloween's pumpkins do, sunk into it a little and
//! easing up and down as it grows and is swept; unlike them it is part of the drift, which lies over the windows, so
//! a window doesn't hide it.
//! A snowman can be picked up and carried. Set down gently it stands again; dropped from a height it breaks into
//! lumps and powder that heap up where they land. A gust clearing the snow breaks each one as it reaches it and
//! blows the pieces away with the rest; a calm clear melts it where it stands.
//! The renderer draws a snowman from its anchor, size, proportions and age alone (`shaders/snowman.frag`).

use std::f32::consts::PI;

use super::{SnowSimulation, SnowmanSprite, gust_front};
use crate::geometry::V2;
use crate::layer::Blend;

/// Where each one stands, as a share of the screen's width, in the order they are built, and its bottom ball's
/// radius in points.
const SPOTS: [(f32, f32); 3] = [(0.78, 30.0), (0.24, 26.0), (0.52, 28.0)];
/// Seconds from the first ball rolling in to the arms: the renderer plays the building from the age.
pub const SNOWMAN_BUILT: f32 = 3.6;
/// Seconds a snowman takes to melt away when the snow is cleared calmly.
const MELT: f32 = 0.7;
/// Where the building is when three balls stacked by hand become a snowman: the balls are in place, the face and the
/// rest are still to come.
const STACKED: f32 = 2.75;
/// A snowman's middle ball and head as shares of its bottom ball, when built from the menu.
const PROPORTIONS: [f32; 3] = [1.0, 0.70, 0.48];
/// At most this many stand on a screen, built from the menu or by hand.
const MOST: usize = 6;
/// A snowman's height as a share of its bottom ball's radius, top of the head to its foot.
pub const SNOWMAN_HEIGHT: f32 = 3.9;
const GRAVITY: f32 = 1500.0;
/// Landing faster than this, in points a second, breaks it: about as fast as from 45 points up.
const BREAKS_AT: f32 = 370.0;

#[derive(Default)]
pub(super) struct Snowmen {
    standing: Vec<Snowman>,
    next: u64,
    /// The pointer's latest place and motion while one is held, for the throw on letting go.
    pointer: Option<(V2, V2)>,
    pub(super) moved: bool,
}

struct Snowman {
    id: u64,
    /// The menu's spot it stands at, or none for one stacked or set down by hand.
    spot: Option<usize>,
    /// Where it stands, as a share of the screen's width, and its bottom ball's radius and proportions.
    share: f32,
    radius: f32,
    radii: [f32; 3],
    /// Where its foot rests, easing after the drift under it, or where it is while carried or falling.
    bottom: f32,
    age: f32,
    /// Seconds since a calm clear began melting it.
    melting: Option<f32>,
    /// Held by the pointer, this far from its foot.
    held: Option<V2>,
    /// Falling after it was let go, at this speed.
    falling: Option<V2>,
}

/// A snowman in the hand, on its way to another display's simulation.
pub struct CarriedSnowman {
    foot: V2,
    radius: f32,
    radii: [f32; 3],
    age: f32,
    offset: V2,
}

impl Snowman {
    /// Its three balls, bottom up: centre and radius, with its foot at `foot`.
    fn balls(&self, foot: V2) -> [(V2, f32); 3] {
        let [a, m, h] = self.radii.map(|r| r * self.radius);
        let c0 = foot + V2::new(0.0, 0.92 * a);
        let cm = c0 + V2::new(0.0, 0.85 * (a + m));
        let ch = cm + V2::new(0.0, 0.85 * (m + h));
        [(c0, a), (cm, m), (ch, h)]
    }
}

impl SnowSimulation {
    /// Builds a snowman at the next free spot; false when all three are taken.
    pub fn build_snowman(&mut self) -> bool {
        if self.snowmen.standing.len() >= MOST {
            return false;
        }
        let Some(spot) = (0..SPOTS.len()).find(|&spot| !self.snowmen.standing.iter().any(|s| s.spot == Some(spot))) else {
            return false;
        };
        let (share, radius) = SPOTS[spot];
        let bottom = self.snowman_target(share, radius);
        self.push_snowman(Some(spot), share, radius, PROPORTIONS, bottom, 0.0);
        true
    }

    /// Three balls stacked by hand become a snowman standing where they are, its foot at `foot`.
    pub(super) fn stand_snowman(&mut self, x: f32, foot: f32, radius: f32, radii: [f32; 3]) {
        if self.snowmen.standing.len() >= MOST {
            self.snowmen.standing.remove(0);
        }
        let share = (x / self.width.max(1.0)).clamp(0.0, 1.0);
        self.push_snowman(None, share, radius, radii, foot, STACKED);
    }

    fn push_snowman(&mut self, spot: Option<usize>, share: f32, radius: f32, radii: [f32; 3], bottom: f32, age: f32) -> u64 {
        let id = self.snowmen.next;
        self.snowmen.next += 1;
        self.snowmen.standing.push(Snowman { id, spot, share, radius, radii, bottom, age, melting: None, held: None, falling: None });
        self.snowmen.moved = true;
        id
    }

    /// The snowmen to draw, before the snow so the drift covers their feet.
    pub fn snowmen(&self) -> Vec<SnowmanSprite> {
        self.snowmen
            .standing
            .iter()
            .map(|s| SnowmanSprite {
                anchor: V2::new(self.width * s.share, s.bottom),
                size: s.radius,
                radii: s.radii,
                age: s.age,
                fade: s.melting.map_or(1.0, |t| (1.0 - t / MELT).max(0.0)),
                seed: (s.id % 7) as f32 + 0.5,
            })
            .collect()
    }

    pub fn has_snowmen(&self) -> bool {
        !self.snowmen.standing.is_empty()
    }

    /// Whether one is being built, carried, falling or melting, which wants the frames of anything in flight.
    pub fn snowmen_busy(&self) -> bool {
        self.snowmen.standing.iter().any(|s| s.age < SNOWMAN_BUILT || s.melting.is_some() || s.held.is_some() || s.falling.is_some())
    }

    /// The snowman under `point`, the front one where they overlap.
    pub fn snowman_at(&self, point: V2) -> Option<u64> {
        self.snowmen
            .standing
            .iter()
            .rev()
            .find(|s| {
                let foot = V2::new(self.width * s.share, s.bottom);
                s.balls(foot).iter().any(|&(c, r)| (point - c).length() <= r + 3.0)
            })
            .map(|s| s.id)
    }

    /// Each snowman's box, left, bottom, right, top: where it takes the pointer.
    pub fn snowman_boxes(&self) -> Vec<[f32; 4]> {
        self.snowmen
            .standing
            .iter()
            .map(|s| {
                let x = self.width * s.share;
                let [_, _, (head, h)] = s.balls(V2::new(x, s.bottom));
                [x - s.radius, s.bottom, x + s.radius, head.y + h]
            })
            .collect()
    }

    pub fn grab_snowman(&mut self, id: u64, point: V2) {
        let width = self.width;
        if let Some(s) = self.snowmen.standing.iter_mut().find(|s| s.id == id) {
            s.held = Some(V2::new(width * s.share, s.bottom) - point);
            s.falling = None;
            s.spot = None;
            self.snowmen.pointer = Some((point, V2::ZERO));
            self.snowmen.moved = true;
        }
    }

    pub fn holding_snowman(&self) -> bool {
        self.snowmen.standing.iter().any(|s| s.held.is_some())
    }

    /// Carries the held snowman after the pointer; it can't go down into the ground.
    pub fn drag_snowman(&mut self, point: V2, dt: f32) {
        let Some(k) = self.snowmen.standing.iter().position(|s| s.held.is_some()) else { return };
        if let Some((last, _)) = self.snowmen.pointer {
            self.snowmen.pointer = Some((point, (point - last) / dt.max(1.0 / 240.0)));
        }
        let (offset, radius) = (self.snowmen.standing[k].held.unwrap_or(V2::ZERO), self.snowmen.standing[k].radius);
        let mut foot = point + offset;
        foot.x = foot.x.clamp(radius, (self.width - radius).max(radius));
        let share = foot.x / self.width.max(1.0);
        let ground = self.snowman_on(share, radius, foot.y);
        let s = &mut self.snowmen.standing[k];
        s.share = share;
        s.bottom = foot.y.max(ground);
        self.snowmen.moved = true;
    }

    /// Lets go of the held snowman: it falls, thrown on a little with the pointer's last motion.
    pub fn release_snowman(&mut self) {
        let throw = self.snowmen.pointer.take().map_or(V2::ZERO, |(_, v)| v * 0.35);
        for s in &mut self.snowmen.standing {
            if s.held.take().is_some() {
                s.falling = Some(V2::new(throw.x.clamp(-500.0, 500.0), throw.y.clamp(-300.0, 500.0)));
                self.snowmen.moved = true;
            }
        }
    }

    /// Lets the held snowman go to another display, still in the hand, as the pointer crosses into it.
    pub fn take_held_snowman(&mut self) -> Option<CarriedSnowman> {
        let k = self.snowmen.standing.iter().position(|s| s.held.is_some())?;
        let s = self.snowmen.standing.remove(k);
        self.snowmen.pointer = None;
        self.snowmen.moved = true;
        Some(CarriedSnowman {
            foot: V2::new(self.width * s.share, s.bottom),
            radius: s.radius,
            radii: s.radii,
            age: s.age,
            offset: s.held.unwrap_or(V2::ZERO),
        })
    }

    /// Takes a snowman carried in from another display, `shift` from its coordinates to these.
    pub fn adopt_held_snowman(&mut self, carried: CarriedSnowman, shift: V2) {
        if self.snowmen.standing.len() >= MOST {
            self.snowmen.standing.remove(0);
        }
        let foot = carried.foot + shift;
        let share = (foot.x / self.width.max(1.0)).clamp(0.0, 1.0);
        let id = self.push_snowman(None, share, carried.radius, carried.radii, foot.y, carried.age);
        if let Some(s) = self.snowmen.standing.iter_mut().find(|s| s.id == id) {
            s.held = Some(carried.offset);
        }
        self.snowmen.pointer = Some((foot - carried.offset, V2::ZERO));
    }

    /// Clearing the snow: a gust breaks each snowman as it reaches it (`tend_snowmen`), a calm clear melts them.
    pub(super) fn knock_snowmen(&mut self, calm: bool) {
        if calm {
            for s in &mut self.snowmen.standing {
                s.melting.get_or_insert(0.0);
            }
        }
    }

    /// Breaks snowman `k` into lumps and powder moving at `velocity`, each ball's worth of snow.
    fn break_snowman(&mut self, k: usize, velocity: V2) {
        let s = self.snowmen.standing.remove(k);
        let foot = V2::new(self.width * s.share, s.bottom);
        for (center, radius) in s.balls(foot) {
            let area = PI * radius * radius * 0.8;
            let spread = V2::new(radius * 0.8, radius * 0.8);
            for _ in 0..3 {
                let at = center + V2::new(self.air.random.range(-0.5, 0.5) * radius, self.air.random.range(-0.5, 0.5) * radius);
                self.air.throw_lump(area * 0.6 / 3.0, at, velocity);
            }
            self.air.release(area * 0.4, Blend::SNOW, 0.0, center, velocity, spread, 0.3);
        }
        self.scattered_until = self.scattered_until.max(self.air.clock + 1.5);
        self.snowmen.moved = true;
    }

    /// Where a snowman's foot belongs: a third of its bottom ball down in the drift, or on the ground where the drift
    /// is shallower.
    fn snowman_target(&self, share: f32, radius: f32) -> f32 {
        let x = self.width * share;
        let layer = &self.layers[self.standing_layer(x, radius)];
        let floor = layer.floor(layer.clamped_index(x));
        let (from, to) = (layer.clamped_index(x - radius * 0.6), layer.clamped_index(x + radius * 0.6));
        let surface = (from..=to).map(|i| layer.surface(i)).sum::<f32>() / (to - from + 1) as f32;
        (surface - radius * 0.35).max(floor)
    }

    /// Where a snowman's foot belongs on whatever lies under it at `bottom`: a third of its bottom ball down in the snow.
    fn snowman_on(&self, share: f32, radius: f32, bottom: f32) -> f32 {
        let x = self.width * share;
        let (li, surface) = self.surface_under(x, radius * 0.6, bottom + radius * 0.35 + 2.0);
        let layer = &self.layers[li];
        (surface - radius * 0.35).max(layer.floor(layer.clamped_index(x)))
    }

    pub(super) fn tend_snowmen(&mut self, dt: f32) {
        if self.snowmen.standing.is_empty() {
            return;
        }
        // A gust breaks each one as its front gets there and blows the pieces on with the snow.
        if let Some(gust) = self.air.gust.filter(|gust| !gust.calm) {
            let front = gust_front(self.width, (self.air.clock - gust.start) as f32, gust.direction, 40.0);
            let width = self.width;
            while let Some(k) =
                self.snowmen.standing.iter().position(|s| s.held.is_none() && (front - width * s.share) * gust.direction >= -s.radius)
            {
                let velocity = V2::new(gust.direction * self.air.random.range(500.0, 900.0), self.air.random.range(120.0, 260.0));
                self.break_snowman(k, velocity);
            }
        }
        // One from the menu stands where the menu put it; one carried or stacked by hand stands on whatever is under
        // it, the top of a window too.
        let targets: Vec<f32> = self
            .snowmen
            .standing
            .iter()
            .map(|s| if s.spot.is_some() { self.snowman_target(s.share, s.radius) } else { self.snowman_on(s.share, s.radius, s.bottom) })
            .collect();
        let mut moved = false;
        let mut broken = Vec::new();
        let width = self.width;
        for (k, (s, target)) in self.snowmen.standing.iter_mut().zip(targets).enumerate() {
            if s.age < SNOWMAN_BUILT {
                s.age = (s.age + dt).min(SNOWMAN_BUILT);
                moved = true;
            }
            if let Some(t) = &mut s.melting {
                *t += dt;
                moved = true;
            }
            if s.held.is_some() {
                continue;
            }
            if let Some(v) = &mut s.falling {
                moved = true;
                v.y -= GRAVITY * dt;
                v.x *= (1.0 - 2.0 * dt).max(0.0);
                s.bottom += v.y * dt;
                let x = (width * s.share + v.x * dt).clamp(s.radius, (width - s.radius).max(s.radius));
                s.share = x / width.max(1.0);
                if s.bottom <= target {
                    if -v.y > BREAKS_AT {
                        broken.push((k, V2::new(v.x * 0.4, -v.y * 0.15)));
                    } else {
                        s.bottom = target;
                        s.falling = None;
                    }
                }
                continue;
            }
            // What it stood on went, as a window closing under it: it falls.
            if target < s.bottom - 6.0 {
                s.falling = Some(V2::ZERO);
                moved = true;
                continue;
            }
            let next = s.bottom + (target - s.bottom) * (dt * 5.0).min(1.0);
            if (next - s.bottom).abs() > 0.01 {
                moved = true;
            }
            s.bottom = next;
        }
        for (k, velocity) in broken.into_iter().rev() {
            self.break_snowman(k, velocity);
        }
        let before = self.snowmen.standing.len();
        self.snowmen.standing.retain(|s| s.melting.is_none_or(|t| t < MELT));
        self.snowmen.moved |= moved || self.snowmen.standing.len() != before;
    }
}

#[cfg(test)]
mod tests {
    use crate::geometry::V2;
    use crate::simulation::SnowSimulation;

    use super::SNOWMAN_BUILT;

    const DT: f32 = 1.0 / 60.0;
    const AWAY: V2 = V2::new(-10_000.0, -10_000.0);

    fn run(simulation: &mut SnowSimulation, seconds: f32) {
        for _ in 0..(seconds / DT) as usize {
            simulation.step(DT, AWAY);
        }
    }

    fn snow(simulation: &SnowSimulation) -> f32 {
        let layer = &simulation.layers[0];
        (0..layer.count()).map(|i| layer.surface(i) - layer.floor(i)).sum()
    }

    #[test]
    fn snowmen_are_built_stand_in_the_drift_and_are_broken_by_a_gust() {
        let mut simulation = SnowSimulation::new(1200.0, 800.0, 0.0);
        for i in 0..simulation.layers[0].count() {
            simulation.layers[0].set(i, 30.0);
        }
        assert!(simulation.snowmen().is_empty() && simulation.is_empty());
        assert!(simulation.build_snowman());
        assert!(!simulation.is_empty(), "a snowman keeps the screen awake");
        run(&mut simulation, 0.5);
        assert!(simulation.snowmen_busy());
        let first = simulation.snowmen()[0];
        assert_eq!(first.anchor.x, 1200.0 * 0.78);
        assert!((first.anchor.y - (30.0 - first.size * 0.35)).abs() < 1.0, "foot at {}", first.anchor.y);
        run(&mut simulation, SNOWMAN_BUILT);
        assert!(!simulation.snowmen_busy() && simulation.snowmen()[0].age == SNOWMAN_BUILT);

        assert!(simulation.build_snowman() && simulation.build_snowman());
        assert!(!simulation.build_snowman(), "three at most from the menu");
        assert_eq!(simulation.snowmen().len(), 3);

        simulation.covers = vec![[0.0, 0.0, 1200.0, 700.0]];
        run(&mut simulation, 2.0);
        assert!(simulation.snowmen().iter().all(|s| s.anchor.y > 0.0), "a window doesn't hide them");
        simulation.covers.clear();

        simulation.blow_away(1.0, false);
        run(&mut simulation, 1.0);
        assert!(simulation.snowmen().is_empty(), "broken by the gust");
    }

    #[test]
    fn a_snowman_is_carried_set_down_and_breaks_when_dropped_from_high() {
        let mut simulation = SnowSimulation::new(1200.0, 800.0, 0.0);
        for i in 0..simulation.layers[0].count() {
            simulation.layers[0].set(i, 20.0);
        }
        simulation.build_snowman();
        run(&mut simulation, SNOWMAN_BUILT + 0.5);
        let s = simulation.snowmen()[0];
        let body = s.anchor + V2::new(0.0, s.size);
        let id = simulation.snowman_at(body).expect("hit on its bottom ball");
        assert!(simulation.snowman_at(V2::new(100.0, 400.0)).is_none());

        // Carried along low and set down: it stands where it was put.
        simulation.grab_snowman(id, body);
        simulation.drag_snowman(body + V2::new(-300.0, 5.0), DT);
        simulation.drag_snowman(body + V2::new(-300.0, 5.0), DT);
        simulation.release_snowman();
        run(&mut simulation, 1.0);
        let moved = simulation.snowmen()[0];
        assert!((moved.anchor.x - (s.anchor.x - 300.0)).abs() < 2.0, "{} vs {}", moved.anchor.x, s.anchor.x - 300.0);

        // Lifted high and dropped: it breaks, and its snow heaps up on the drift.
        let before = snow(&simulation);
        let body = moved.anchor + V2::new(0.0, moved.size);
        simulation.grab_snowman(simulation.snowman_at(body).expect("hit"), body);
        simulation.drag_snowman(body + V2::new(0.0, 300.0), DT);
        simulation.drag_snowman(body + V2::new(0.0, 300.0), DT);
        simulation.release_snowman();
        run(&mut simulation, 3.0);
        assert!(simulation.snowmen().is_empty(), "broken");
        assert!(snow(&simulation) > before + 50.0, "heaped up: {before} -> {}", snow(&simulation));
    }
}
