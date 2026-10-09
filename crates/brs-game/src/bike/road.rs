//! The road of the ride on the bike: five lanes of chips, a row of them every
//! hundred metres, and a place on it that goes along at a speed
//! (`docs/BIKE_NOTES.md`, "The road"). Lengths are centimetres, speeds km/h.

use bevy::math::Vec3;

use super::tables::{RoadTables, LANES, ROW};

/// A chip's marks (the line table's word for its lane).
pub mod mark {
    /// A lane that is to be left: one that may change lanes is turned off it.
    pub const TO_BE_LEFT: u32 = 1;
    /// The road's edges, ridden on as walls.
    pub const LEFT_BANK: u32 = 2;
    pub const RIGHT_BANK: u32 = 4;
    /// No lane is changed from here.
    pub const NO_TURNING: u32 = 0x10;
    /// A lane the pad cannot take her on to.
    pub const SHUT: u32 = 0x20;
    /// What the camera is told of the lane she is in.
    pub const FOR_THE_CAMERA: u32 = 0xff00;
    /// The display's two signs.
    pub const SIGNS: [u32; 2] = [0x10000, 0x20000];
}

/// A piece of a lane (0x80 bytes in the original). Every chip the road's
/// builder makes is straight, so the turning the original also has code for
/// (a chip's flag 8, 0x142144's other half) has nothing to be for here.
#[derive(Clone, Debug)]
pub struct Chip {
    pub left: Option<usize>,
    pub right: Option<usize>,
    pub before: Option<usize>,
    pub after: Option<usize>,
    pub lane: usize,
    pub row: usize,
    pub length: f32,
    pub begins: Vec3,
    pub ends: Vec3,
    pub marks: u32,
}

/// The road: its chips by number (a row's five follow one another), and the
/// first and last chip of each lane.
#[derive(Clone, Debug, Default)]
pub struct Road {
    pub chips: Vec<Chip>,
    firsts: [Option<usize>; LANES],
    lasts: [Option<usize>; LANES],
}

impl Road {
    /// Builds the road of a course's tables (0x139368): for each row a chip a
    /// lane, each a row long and straight on from the one before it.
    pub fn build(tables: &RoadTables) -> Road {
        let mut road = Road::default();
        for (row, kind) in tables.rows.iter().enumerate() {
            for lane in 0..LANES {
                let number = road.chips.len();
                let before = road.lasts[lane];
                let begins = match before {
                    Some(before) => road.chips[before].ends,
                    None => Vec3::new(tables.first_lane - tables.lane_width * lane as f32, 0.0, 0.0),
                };
                road.chips.push(Chip {
                    left: (lane > 0).then(|| number - 1),
                    right: (lane + 1 < LANES).then_some(number + 1),
                    before,
                    after: None,
                    lane,
                    row,
                    length: ROW,
                    begins,
                    ends: begins + Vec3::Z * ROW,
                    marks: tables.lines[kind.line][lane],
                });
                if let Some(before) = before {
                    road.chips[before].after = Some(number);
                }
                road.firsts[lane].get_or_insert(number);
                road.lasts[lane] = Some(number);
            }
        }
        road
    }

    /// The chip `steps` on from a lane's first, or back from its last; the end
    /// of the lane if it is not so long (0x138E2C).
    pub fn chip_of(&self, lane: usize, from_the_end: bool, steps: usize) -> Option<usize> {
        let mut chip = (*if from_the_end { &self.lasts } else { &self.firsts }).get(lane).copied().flatten()?;
        for _ in 0..steps {
            match if from_the_end { self.chips[chip].before } else { self.chips[chip].after } {
                Some(next) => chip = next,
                None => break,
            }
        }
        Some(chip)
    }

    /// Where a chip is at a part of the way along it, 0 to 1 (0x142144).
    pub fn at(&self, chip: usize, part: f32) -> Vec3 {
        let chip = &self.chips[chip];
        chip.begins.lerp(chip.ends, part.clamp(0.0, 1.0))
    }

    /// The marks of a chip, or none of no chip.
    pub fn marks(&self, chip: Option<usize>) -> u32 {
        chip.map_or(0, |chip| self.chips[chip].marks)
    }
}

/// The colour of the light at a place by the road (0x138F68): that of the first
/// zone of its row's pattern that the place is in, or the open road's.
pub fn light_at(tables: &RoadTables, place: Vec3) -> [f32; 3] {
    for (row, kind) in tables.rows.iter().enumerate() {
        let middle = row as f32 * ROW + ROW / 2.0;
        if place.z < middle - ROW / 2.0 || place.z > middle + ROW / 2.0 {
            continue;
        }
        let Some(zones) = usize::try_from(kind.patterns[0]).ok().and_then(|pattern| tables.zones.get(pattern)) else { continue };
        for zone in zones.iter().filter(|zone| zone.kind >= 1) {
            // The place as the zone's box has it: about its middle, turned back.
            let from = place - (Vec3::from(zone.place) + Vec3::Z * middle);
            let own = bevy::math::Quat::from_rotation_y(-zone.heading.to_radians()) * from;
            if own.abs().cmple(Vec3::from(zone.half)).all() {
                return tables.lights[zone.kind as usize];
            }
        }
    }
    tables.lights[0]
}

/// A place on the road (20 bytes in the original): the chip, how far along it
/// and which part of the way, the speed it goes on at and how far it has come.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Place {
    pub chip: Option<usize>,
    pub along: f32,
    pub part: f32,
    pub speed: f32,
    pub come: f32,
}

impl Place {
    /// Puts it at a part of the way along its chip, going on to the chips after
    /// or before for a part beyond 1 or under 0 (0x138230, 1).
    pub fn set_part(&mut self, road: &Road, mut part: f32) {
        let Some(mut chip) = self.chip else {
            (self.part, self.along) = (0.0, 0.0);
            return;
        };
        while part > 1.0 {
            match road.chips[chip].after {
                Some(after) => (chip, part) = (after, part - 1.0),
                None => part = 1.0,
            }
        }
        while part < 0.0 {
            match road.chips[chip].before {
                Some(before) => (chip, part) = (before, part + 1.0),
                None => part = 0.0,
            }
        }
        (self.chip, self.part, self.along) = (Some(chip), part, road.chips[chip].length * part);
    }

    /// Puts it on a chip, the same part of the way along (0x13805C).
    pub fn put(&mut self, road: &Road, chip: usize) {
        self.chip = Some(chip);
        self.set_part(road, self.part);
    }

    /// Takes it on by a length, from chip to chip; it stops at the road's ends
    /// (0x138138). The part of the way is not kept up by this.
    pub fn go(&mut self, road: &Road, by: f32) {
        let Some(mut chip) = self.chip else { return };
        self.along += by;
        while self.along > road.chips[chip].length {
            match road.chips[chip].after {
                Some(after) => {
                    self.along -= road.chips[chip].length;
                    chip = after;
                }
                None => self.along = road.chips[chip].length,
            }
        }
        while self.along < 0.0 {
            match road.chips[chip].before {
                Some(before) => {
                    chip = before;
                    self.along += road.chips[chip].length;
                }
                None => self.along = 0.0,
            }
        }
        self.chip = Some(chip);
    }

    /// A frame of going on at its speed (0x137EE4).
    pub fn step(&mut self, road: &Road, time: f32) {
        if self.chip.is_none() {
            return;
        }
        let by = self.speed * 1000.0 * 100.0 / 60.0 / 60.0 * time;
        self.come += by;
        self.go(road, by);
        self.part = self.along / road.chips[self.chip.unwrap()].length;
    }

    /// Takes it to the chip beside, if there is one (0x137F84).
    pub fn beside(&mut self, road: &Road, right: bool) {
        let Some(chip) = self.chip else { return };
        if let Some(beside) = if right { road.chips[chip].right } else { road.chips[chip].left } {
            self.put(road, beside);
        }
    }

    /// Where it is (0x138088).
    pub fn position(&self, road: &Road) -> Vec3 {
        self.chip.map_or(Vec3::ZERO, |chip| road.at(chip, self.part))
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::bike::tables::{self, course_pack, COURSE_ROAD};

    pub fn normal() -> Option<Road> {
        let data = tables::tests::data()?;
        Some(Road::build(&RoadTables::read(&tables::tests::script(&data, &course_pack("normal"), COURSE_ROAD)).unwrap()))
    }

    #[test]
    fn the_road_is_five_straight_lanes_of_a_hundred_rows() {
        let Some(road) = normal() else { return };
        assert_eq!(road.chips.len(), 500);
        // Lane 2 is the middle; lane 0 is five metres to its left.
        let middle = road.chip_of(2, false, 0).unwrap();
        assert_eq!((road.chips[middle].begins, road.chips[middle].ends), (Vec3::ZERO, Vec3::new(0.0, 0.0, 10000.0)));
        assert_eq!(road.chips[road.chip_of(0, false, 0).unwrap()].begins.x, 500.0);
        assert_eq!(road.chips[road.chip_of(4, false, 0).unwrap()].begins.x, -500.0);
        // The chip the course's script starts her on and the goal's.
        let start = road.chip_of(2, false, 2).unwrap();
        assert_eq!((start, road.chips[start].row, road.chips[start].lane), (12, 2, 2));
        assert_eq!(road.chips[road.chip_of(2, false, 77).unwrap()].begins.z, 770000.0);
        // Beside it are lanes 1 and 3; the lane's ends are its ends.
        assert_eq!((road.chips[start].left, road.chips[start].right), (Some(11), Some(13)));
        assert_eq!(road.chip_of(2, false, 500), road.chip_of(2, true, 0));
        assert_eq!(road.chip_of(2, true, 99), Some(middle));
        // The outer lanes of an open stretch are shut and to be left.
        assert_eq!(road.marks(road.chip_of(0, false, 0)) & 0xff, mark::TO_BE_LEFT | mark::SHUT);
        assert_eq!(road.marks(road.chip_of(2, false, 0)) & 0xff, 0);
    }

    #[test]
    fn a_place_goes_on_from_chip_to_chip_and_stops_at_the_roads_end() {
        let Some(road) = normal() else { return };
        let mut place = Place { chip: road.chip_of(2, false, 2), speed: 360.0, ..Place::default() };
        // A second at 360 km/h is a hundred metres: a chip.
        for _ in 0..30 {
            place.step(&road, 1.0 / 30.0);
        }
        assert!((place.position(&road).z - 30000.0).abs() < 1.0 && (place.come - 10000.0).abs() < 1.0, "{place:?}");
        // A part past the chip's end is on the chips after.
        place.put(&road, road.chip_of(2, false, 3).unwrap());
        place.set_part(&road, 2.5);
        assert_eq!((place.chip.map(|chip| road.chips[chip].row), place.part, place.along), (Some(5), 0.5, 5000.0));
        assert_eq!(place.position(&road), Vec3::new(0.0, 0.0, 55000.0));
        place.set_part(&road, -1.25);
        assert_eq!((place.chip.map(|chip| road.chips[chip].row), place.part), (Some(3), 0.75));
        // Beside: the same part of the way along the next lane.
        place.beside(&road, false);
        assert_eq!(place.position(&road), Vec3::new(250.0, 0.0, 37500.0));
        // Back beyond the road's beginning, and on beyond its end.
        place.go(&road, -1.0e6);
        assert_eq!((place.chip.map(|chip| road.chips[chip].row), place.along), (Some(0), 0.0));
        place.go(&road, 2.0e6);
        assert_eq!((place.chip.map(|chip| road.chips[chip].row), place.along), (Some(99), 10000.0));
    }
}
