//! Snowmen, built from the menu: three balls of snow, the bottom one rolled in along the drift and the other two
//! dropped onto it, then coal eyes, a carrot nose, buttons, a scarf and twig arms. Each stands in the drift on the
//! ground the way Halloween's pumpkins do, sunk into it a little and easing up and down as it grows and is swept;
//! unlike them it is part of the drift, which lies over the windows, so a window doesn't hide it. A gust that
//! clears the snow knocks them over and they crumble.
//! The renderer draws a snowman from its anchor, size and age alone (`shaders/snowman.frag`).

use super::{SnowSimulation, SnowmanSprite};
use crate::geometry::V2;

/// Where each one stands, as a share of the screen's width, in the order they are built, and its bottom ball's
/// radius in points.
const SPOTS: [(f32, f32); 3] = [(0.78, 30.0), (0.24, 26.0), (0.52, 28.0)];
/// Seconds from the first ball rolling in to the arms: the renderer plays the building from the age.
pub const SNOWMAN_BUILT: f32 = 3.6;
/// Seconds a knocked-over snowman takes to crumble away.
const CRUMBLE: f32 = 0.7;
/// Where the building is when three balls stacked by hand become a snowman: the balls are in place, the face and the
/// rest are still to come.
const STACKED: f32 = 2.75;
/// A snowman's middle ball and head as shares of its bottom ball, when built from the menu.
const PROPORTIONS: [f32; 3] = [1.0, 0.70, 0.48];
/// At most this many stand on a screen, built from the menu or by hand.
const MOST: usize = 6;
/// A snowman's height as a share of its bottom ball's radius, top of the head to its foot.
pub const SNOWMAN_HEIGHT: f32 = 3.9;

#[derive(Default)]
pub(super) struct Snowmen {
    standing: Vec<Snowman>,
    pub(super) moved: bool,
}

struct Snowman {
    /// The menu's spot it stands at, or none for one stacked by hand.
    spot: Option<usize>,
    /// Where it stands, as a share of the screen's width, and its bottom ball's radius and proportions.
    share: f32,
    radius: f32,
    radii: [f32; 3],
    /// Where its foot rests, easing after the drift under it.
    bottom: f32,
    age: f32,
    /// Seconds since a gust knocked it over, while it crumbles.
    knocked: Option<f32>,
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
        self.snowmen.standing.push(Snowman { spot: Some(spot), share, radius, radii: PROPORTIONS, bottom, age: 0.0, knocked: None });
        self.snowmen.moved = true;
        true
    }

    /// Three balls stacked by hand become a snowman standing where they are, its foot at `foot`.
    pub(super) fn stand_snowman(&mut self, x: f32, foot: f32, radius: f32, radii: [f32; 3]) {
        if self.snowmen.standing.len() >= MOST {
            self.snowmen.standing.remove(0);
        }
        let share = (x / self.width.max(1.0)).clamp(0.0, 1.0);
        self.snowmen.standing.push(Snowman { spot: None, share, radius, radii, bottom: foot, age: STACKED, knocked: None });
        self.snowmen.moved = true;
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
                fade: s.knocked.map_or(1.0, |t| (1.0 - t / CRUMBLE).max(0.0)),
                seed: s.share * 7.0,
            })
            .collect()
    }

    pub fn has_snowmen(&self) -> bool {
        !self.snowmen.standing.is_empty()
    }

    /// Whether one is being built or crumbling, which wants the frames of anything in flight.
    pub fn snowmen_busy(&self) -> bool {
        self.snowmen.standing.iter().any(|s| s.age < SNOWMAN_BUILT || s.knocked.is_some())
    }

    /// A gust clearing the snow knocks them all over.
    pub(super) fn knock_snowmen(&mut self) {
        for s in &mut self.snowmen.standing {
            s.knocked.get_or_insert(0.0);
        }
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

    pub(super) fn tend_snowmen(&mut self, dt: f32) {
        if self.snowmen.standing.is_empty() {
            return;
        }
        let targets: Vec<f32> = self.snowmen.standing.iter().map(|s| self.snowman_target(s.share, s.radius)).collect();
        let mut moved = false;
        for (s, target) in self.snowmen.standing.iter_mut().zip(targets) {
            if s.age < SNOWMAN_BUILT {
                s.age = (s.age + dt).min(SNOWMAN_BUILT);
                moved = true;
            }
            if let Some(t) = &mut s.knocked {
                *t += dt;
                moved = true;
            }
            let next = s.bottom + (target - s.bottom) * (dt * 5.0).min(1.0);
            if (next - s.bottom).abs() > 0.01 {
                moved = true;
            }
            s.bottom = next;
        }
        let before = self.snowmen.standing.len();
        self.snowmen.standing.retain(|s| s.knocked.is_none_or(|t| t < CRUMBLE));
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

    #[test]
    fn snowmen_are_built_stand_in_the_drift_and_are_knocked_over_by_a_gust() {
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
        assert!(!simulation.build_snowman(), "three at most");
        assert_eq!(simulation.snowmen().len(), 3);

        simulation.covers = vec![[0.0, 0.0, 1200.0, 700.0]];
        run(&mut simulation, 2.0);
        assert!(simulation.snowmen().iter().all(|s| s.anchor.y > 0.0), "a window doesn't hide them");
        simulation.covers.clear();

        simulation.blow_away(1.0, false);
        run(&mut simulation, 1.0);
        assert!(simulation.snowmen().is_empty(), "crumbled");
    }
}
