use eframe::{
    egui::{self, Ui},
    epaint::{Color32, Pos2, RectShape}, wgpu::Color,
};
use maze::generation::{actual_prim_generator::GappedPrimGenerator, Generator};
use maze::GeneratorOptions;
use maze::{execution::Tile, Point};
use maze::{
    execution::{
        threaded_controller::{ThreadedControllerWrapper, ThreadedRobotProgress},
        threaded_robot::ThreadedRobot,
        Controller,
    },
    generation::Maze,
};
use std::thread;
use std::{
    borrow::BorrowMut,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, Condvar, Mutex, RwLock,
    },
};

use crate::exercise_1::DumboController;

pub fn create_app() -> MazeApp {
    let active = Arc::from(Mutex::from(false));
    let thread_delay = Arc::from(Mutex::from(100));
    let (tx, rx) = channel();
    let latest_robot_update = Arc::from(Mutex::from(None));

    let mut wrap = ThreadedControllerWrapper::<DumboController>::new(
        active.clone(),
        thread_delay.clone(),
        latest_robot_update.clone(),
    );

    let maze_ref = ThreadedRobot::get_maze(wrap.get_robot());
    // let th_maze_ref = maze_ref.clone();

    let pair = Arc::new((Mutex::new(false), Condvar::new()));
    let th_pair = pair.clone();

    thread::spawn(move || {
        wrap.set_sender(tx);

        loop {
            let (lock, cvar) = &*th_pair;
            let mut started = lock.lock().unwrap();
            while !*started {
                started = cvar.wait(started).unwrap();
            }

            // wrap.set_maze_ref(th_maze_ref);
            wrap.start();

            let mut started = lock.lock().unwrap();
            *started = false;
            cvar.notify_all();
        }
    });

    let app = MazeApp::new(
        active.clone(),
        thread_delay.clone(),
        rx,
        latest_robot_update.clone(),
        maze_ref,
        pair,
    );
    app
}

pub struct MazeApp {
    active: Arc<Mutex<bool>>,
    thread_delay: Arc<Mutex<i32>>,
    thread_delay_local: i32,
    reciever: Receiver<ThreadedRobotProgress>,
    latest_robot_update: Arc<Mutex<Option<ThreadedRobotProgress>>>,
    maze_options: Box<GeneratorOptions>,
    maze_ref: Arc<RwLock<Maze<Tile>>>,
    control: Arc<(Mutex<bool>, Condvar)>,
}

impl MazeApp {
    fn new(
        active: Arc<Mutex<bool>>,
        thread_delay: Arc<Mutex<i32>>,
        reciever: Receiver<ThreadedRobotProgress>,
        latest_robot_update: Arc<Mutex<Option<ThreadedRobotProgress>>>,
        maze_ref: Arc<RwLock<Maze<Tile>>>,
        control: Arc<(Mutex<bool>, Condvar)>,
    ) -> Self {
        Self {
            active,
            thread_delay,
            thread_delay_local: 100,
            reciever,
            latest_robot_update,
            maze_ref,
            maze_options: Default::default(),
            control,
        }
    }
}

impl eframe::App for MazeApp {
    fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame) {
        egui::SidePanel::left("MazePanel")
            .min_width(600.0)
            .resizable(false)
            .show(ctx, |ui| {
                // Number of points each square of maze should occupy
                let scale = ui.available_width() / self.maze_ref.read().unwrap().get_width() as f32;

                let robot_finished = match self.latest_robot_update.try_lock() {
                    Ok(res) => match &*res {
                        Some(prog) => prog.finished,
                        None => true,
                    },
                    Err(_) => true,
                };

                MazeApp::draw_maze(ui, self.maze_ref.clone(), scale);

                // println!("finished {:?}", robot_finished);
                // if not finished draw frames in buffer
                if !robot_finished {
                    loop {
                        match self.reciever.try_recv() {
                            Ok(data) => {
                                MazeApp::draw_progress(ui, data, self.maze_ref.clone(), scale);
                            }
                            Err(_) => break,
                        }
                    }
                    // then request a repaint so we dont stall for ages
                    ctx.request_repaint();
                }
                // else if we're done just draw final frame
                else {
                    match self.latest_robot_update.try_lock() {
                        Ok(data) => match &*data {
                            Some(data) => {
                                MazeApp::draw_progress(ui, data.clone(), self.maze_ref.clone(), scale)
                            }
                            None => {
                                MazeApp::draw_progress(
                                    ui,
                                    ThreadedRobotProgress {
                                        finished: true,
                                        robot_head: maze::execution::Heading::East,
                                        robot_pos: Point(1, 1),
                                        target_loc: match self.maze_ref.read() {
                                            Ok(some) => some.get_finish(),
                                            Err(_) => Point(1, 1),
                                        },
                                    },
                                    self.maze_ref.clone(),
                                    scale,
                                );
                            }
                        },
                        Err(e) => println!("{:?} err in read latest robot", e),
                    }
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Maze App");

            ui.label("Width: ");
            ui.add(egui::Slider::new(&mut self.maze_options.width, 0..=120));
            ui.label("Height: ");
            ui.add(egui::Slider::new(&mut self.maze_options.height, 0..=120));

            if ui.button("Generate Maze").clicked() {
                let mut generator = GappedPrimGenerator::new();
                generator.set_options(*self.maze_options);

                match self.maze_ref.write() {
                    Ok(mut maze) => {
                        *maze = generator.generate_maze();
                    }
                    Err(err) => println!("Couldn't gen maze {:?}", err),
                }
            }

            ui.label("Delay");


            let slider_res = ui.add(egui::Slider::new(
                &mut self.thread_delay_local,
                0..=1000,
            ));

            if slider_res.changed() {
                *self.thread_delay.lock().unwrap() = self.thread_delay_local;
            }

            if ui.button("Start Maze").clicked() {
                match self.active.lock() {
                    Ok(mut data) => *data = true,
                    Err(_) => (),
                }

                let (lock, cvar) = &*self.control;
                let mut started = lock.lock().unwrap();
                *started = true;
                cvar.notify_one();
            }

            if ui.button("Reset Maze").clicked() {
                match self.active.lock() {
                    Ok(mut data) => *data = false,
                    Err(_) => (),
                }
            }
        });
    }
}

impl MazeApp {
    pub fn draw_maze(
        ui: &Ui,
        maze: Arc<RwLock<Maze<Tile>>>,
        scale: f32,
    ) {
        use eframe::epaint::{Rect, Shape};
        use egui::Rounding;

        let mut shapes = Vec::new();
        let maze_data = match maze.read() {
            Ok(val) => val.clone(),
            Err(_) => unreachable!(),
        };

        // Draw the maze
        for y in 0..maze_data.get_height() {
            for x in 0..maze_data.get_width() {
                shapes.push(Shape::Rect(RectShape {
                    rect: Rect {
                        min: Pos2 {
                            x: x as f32 * scale,
                            y: y as f32 * scale,
                        },
                        max: Pos2 {
                            x: (x + 1) as f32 * scale,
                            y: (y + 1) as f32 * scale,
                        },
                    },
                    fill: match maze_data.get_cell(Point(x, y)) {
                        Some(Tile::Wall) => Color32::DARK_GRAY,
                        _ => Color32::GRAY
                    },
                    rounding: Rounding::none(),
                    stroke: Default::default(),
                }));
            }
        }

        // Render out to the screen
        ui.painter().extend(shapes);
    }

    fn draw_progress(
        ui: &Ui,
        prog: ThreadedRobotProgress,
        maze: Arc<RwLock<Maze<Tile>>>,
        scale: f32,
    ) {
        use eframe::epaint::{PathShape, Rect, Shape};
        use egui::Rounding;
        use maze::execution::Heading;

        let mut shapes = Vec::new();
        let maze_data = match maze.read() {
            Ok(val) => val.clone(),
            Err(_) => unreachable!(),
        };

        // Draw the maze
        for y in 0..maze_data.get_height() {
            for x in 0..maze_data.get_width() {

                if *maze_data.get_cell(Point(x, y)).unwrap_or(&Tile::Wall) == Tile::BeenBefore {
                    shapes.push(Shape::Rect(RectShape {
                        rect: Rect {
                            min: Pos2 {
                                x: x as f32 * scale,
                                y: y as f32 * scale,
                            },
                            max: Pos2 {
                                x: (x + 1) as f32 * scale,
                                y: (y + 1) as f32 * scale,
                            },
                        },
                        fill: Color32::LIGHT_GRAY,
                        rounding: Rounding::none(),
                        stroke: Default::default(),
                    }));
                }
            }
        }

        // Draw the finish square
        shapes.push(Shape::Rect(RectShape {
            rect: Rect {
                min: Pos2 {
                    x: prog.target_loc.get_x() as f32 * scale,
                    y: prog.target_loc.get_y() as f32 * scale,
                },
                max: Pos2 {
                    x: (prog.target_loc.get_x() + 1) as f32 * scale,
                    y: (prog.target_loc.get_y() + 1) as f32 * scale,
                },
            },
            fill: Color32::GREEN,
            rounding: Rounding::none(),
            stroke: Default::default(),
        }));
        // println!("{:?}", maze_data.get_finish());

        // Draw the robot
        shapes.push(Shape::Path(PathShape {
            points: (match prog.robot_head {
                Heading::North => [(0.0, 1.0), (0.5, 0.0), (1.0, 1.0)],
                Heading::East => [(0.0, 0.0), (1.0, 0.5), (0.0, 1.0)],
                Heading::South => [(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)],
                Heading::West => [(1.0, 0.0), (1.0, 1.0), (0.0, 0.5)],
            })
            .iter()
            .map(|(x, y)| Pos2 {
                x: (prog.robot_pos.get_x() as f32 + x) * scale,
                y: (prog.robot_pos.get_y() as f32 + y) * scale,
            })
            .collect(),
            closed: true,
            fill: Color32::BLUE,
            stroke: Default::default(),
        }));

        // Render out to the screen
        ui.painter().extend(shapes);
    }
}
