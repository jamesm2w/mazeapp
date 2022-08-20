// Dumbo Controller
// Can we have a robot which chooses direction randomly but avoids walls

use maze::execution::polled_controller::PolledController;
use maze::execution::{Facing, Robot, Tile};
use rand::Rng;

#[derive(Default)]
pub struct DumboController;

impl<R: Robot<Tiles = Tile>> PolledController<R> for DumboController {
    fn control_robot(&mut self, robot: &mut R) {
        let mut random_gen = rand::thread_rng();
        
        'rand: loop {
            let direction = match random_gen.gen_range(0..=3) {
                0 => Facing::Ahead,
                1 => Facing::Left,
                2 => Facing::Right,
                _ => Facing::Behind,
            };

            match robot.look(direction) {
                Tile::Wall => continue 'rand,
                _ => { robot.face(direction); break 'rand; }
            }
        }
    }
}