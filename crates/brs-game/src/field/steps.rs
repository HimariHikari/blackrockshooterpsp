//! Her steps: which foot comes down when.
//!
//! Only her scripts have an `update_time`, and the field's resident script
//! (`fld_resident.bin/05.sscr`, `fchr_z_rsd_update_foot_se`) gives it a body: while
//! she walks or runs a foot comes down as her animation passes two of its frames,
//! and the engine plays the sound that goes with the foot and with the kind of
//! ground under her (0xFC954). See "Sound" in `docs/FIELD_NOTES.md`.

use super::scene::status;

/// The frames of her animation at which a foot comes down, the first foot and the
/// second: walking or running, and in her twelfth status. The script has them as
/// times, of 1024 to a frame: 0x1800 and 0x5800, 0x5000 and 0x7800.
const MOVING: [f32; 2] = [6.0, 22.0];
const BRAKE: [f32; 2] = [20.0, 30.0];

/// The feet that come down as her animation goes from frame `before` to frame
/// `after` in the status she is in: a foot does when its frame is not behind
/// `before` and is behind `after`.
pub fn feet(status: i32, before: f32, after: f32) -> impl Iterator<Item = usize> {
    let frames = match status {
        status::WALK | status::RUN => Some(MOVING),
        status::BRAKE => Some(BRAKE),
        _ => None,
    };
    frames.into_iter().flatten().enumerate().filter(move |&(_, frame)| frame >= before && frame < after).map(|(foot, _)| foot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn down(status: i32, before: f32, after: f32) -> Vec<usize> {
        feet(status, before, after).collect()
    }

    #[test]
    fn a_foot_comes_down_as_her_animation_passes_its_frame() {
        // Two frames of her animation go by in a frame of the game.
        assert_eq!(down(status::RUN, 2.0, 4.0), [] as [usize; 0]);
        assert_eq!(down(status::RUN, 4.0, 6.0), [] as [usize; 0]);
        assert_eq!(down(status::RUN, 6.0, 8.0), [0]);
        assert_eq!(down(status::RUN, 5.0, 7.0), [0]);
        assert_eq!(down(status::RUN, 8.0, 10.0), [] as [usize; 0]);
        assert_eq!(down(status::WALK, 21.0, 23.0), [1]);
        // Each foot once in a turn of the animation, which is 32 frames.
        let turn: Vec<usize> = (0..16).flat_map(|step| down(status::RUN, step as f32 * 2.0, step as f32 * 2.0 + 2.0)).collect();
        assert_eq!(turn, [0, 1]);
        // Standing she makes no step; her twelfth status has frames of its own.
        assert_eq!(down(status::STAND, 5.0, 7.0), [] as [usize; 0]);
        assert_eq!(down(status::BRAKE, 5.0, 7.0), [] as [usize; 0]);
        assert_eq!(down(status::BRAKE, 19.0, 21.0), [0]);
        assert_eq!(down(status::BRAKE, 29.0, 31.0), [1]);
    }

    #[test]
    fn each_kind_of_ground_has_a_sound_for_each_foot() {
        use crate::data::GameData;
        use crate::field::tables::FieldTables;

        let Ok(data) = GameData::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../game/unpacked")) else {
            eprintln!("skipped: game data not unpacked");
            return;
        };
        let tables = FieldTables::load(&data).unwrap();
        // Ten kinds: the sounds from 0x14 up, two by two, and a last kind whose
        // sound is the bank's first.
        assert_eq!(tables.steps.len(), 10);
        assert_eq!((tables.step_sound(0, 0), tables.step_sound(0, 1)), (Some(0x15), Some(0x14)));
        assert_eq!((tables.step_sound(8, 0), tables.step_sound(8, 1)), (Some(0x25), Some(0x24)));
        assert_eq!(tables.step_sound(9, 0), Some(0));
        assert_eq!(tables.step_sound(10, 0), None);
    }
}
