extern crate maze;

use std::io::Empty;
use std::sync::mpsc::TryRecvError;
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::Mutex;
use std::task::Poll;
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use eframe::epaint::Color32;
use eframe::epaint::PathShape;
use eframe::epaint::Pos2;
use eframe::epaint::Shape;
use eframe::epaint::Stroke;
use eframe::epaint::Vec2;
use eframe::wgpu::Color;
use eframe::NativeOptions;
use maze::execution::polled_controller::PolledControllerWrapper;
use maze::execution::robot;
use maze::execution::robot::DefaultRobot;
use maze::execution::Controller;
use maze::execution::Facing;
use maze::execution::Heading;
use maze::execution::Robot;
use maze::execution::Tile;
use maze::generation::actual_prim_generator::GappedPrimGenerator;
use maze::generation::Generator;
use maze::generation::Maze;
use maze::GeneratorOptions;
use maze::Point;

pub mod exercise_1;

use exercise_1::DumboController;

// fn main() {
//     println!("Hello, world!");

//     let mut gen = PrimGenerator::new();
//     let maze: Maze<Tile> = gen.generate_maze();
//     println!("{:?}", maze.get_cell(maze.get_finish()));
//     println!("Robot Start Pos {:?}", maze.get_start());
//     println!("Robot End Pos {:?}", maze.get_finish());

//     let mut wrapp = PolledControllerWrapper::<DefaultRobot, DumboController>::new();

//     wrapp.set_maze(maze);
//     wrapp.set_delay(10000);
//     print!("{esc}[2J{esc}[H", esc = 27 as char);
//     wrapp.set_poll_callback(Box::new(|robot: &DefaultRobot| {
//         robot.print();
//     }));
//     wrapp.start();

//     // print!("{esc}[2J{esc}[H", esc = 27 as char);
//     // println!("Robot reached goal.");
// }
use eframe::egui;

#[derive(Debug)]
struct Message {
    maze: Maze<Tile>,
    start: bool,
}

#[derive(Debug)]
struct RobotProgress {
    robot_pos: Point,
    robot_facing: Heading,
    target: Point,
    maze: Maze<Tile>
}

fn main() {
    let mut options = eframe::NativeOptions::default();
    options.initial_window_size = Some(Vec2 { x: 800.0, y: 600.0 });
    options.vsync = false;
    let (tx, rx) = channel::<Message>();

    let (ttx, trx) = channel();

    let handle = thread::spawn(move || {
        match rx.recv() {
            Ok(msg) => {
                let mut wrap = PolledControllerWrapper::<DefaultRobot, DumboController>::new();
                wrap.set_maze(msg.maze);
                wrap.set_delay(0);

                // print!("{esc}[2J", esc = 27 as char);
                // wrap.get_robot().print();

                wrap.set_poll_callback(Box::new(move |robot| {
                    let res = ttx.send(RobotProgress {
                        robot_pos: robot.get_location().clone(),
                        robot_facing: robot.get_heading(),
                        target: robot.get_goal_location(),
                        maze: robot.get_maze().clone()
                    });
                    match res {
                        Ok(_) => (),
                        Err(err) => println!("err {:?}", err),
                    }
                }));
                wrap.start();
                // ttx.send(RobotProgress { robot_pos: Point(0,0), robot_facing: Heading::North, target: Point(0, 0) });
            }
            Err(err) => {
                println!("{:?}", err);
            }
        }
    });

    eframe::run_native(
        "Maze App",
        options,
        Box::new(|_cc| Box::new(MyApp::new(handle, tx, trx))),
    );
}

struct MyApp {
    width: i32,
    height: i32,
    maze: Maze<Tile>,
    controller_thread: JoinHandle<()>,
    transmitter: Sender<Message>,
    receiver: Receiver<RobotProgress>,
    prev_robot_prog: Option<RobotProgress>,
    active: bool,
}

impl MyApp {
    fn new(
        controller_thread_handle: JoinHandle<()>,
        transmitter: Sender<Message>,
        receiver: Receiver<RobotProgress>,
    ) -> Self {
        Self {
            width: 15,
            height: 15,
            maze: Maze::new(15, 15),
            controller_thread: controller_thread_handle,
            transmitter,
            receiver,
            prev_robot_prog: Some(RobotProgress { robot_pos: Point(1, 1), robot_facing: Heading::East, target: Point(13, 13), maze: Maze::new(15, 15) }),
            active: false,
        }
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::SidePanel::left("MazePanel")
            .min_width(600.0)
            .resizable(false)
            .show(ctx, |ui| {
                // Number of points each square of maze should occupy
                let scale = ui.available_width() / self.maze.get_width() as f32;
                match self.receiver.try_recv() {
                    Ok(data) => {
                        self.prev_robot_prog = Some(data);
                    }
                    Err(err) => match err {
                        TryRecvError::Empty => (),
                        TryRecvError::Disconnected => { self.active = false; } // maybe handle this but why
                    }
                };

                self.prev_robot_prog.iter().for_each(|data| {
                    for (i, r) in data.maze.get_grid().iter().enumerate() {
                        for (j, t) in r.iter().enumerate() {
                            ui.painter().rect_filled(
                                eframe::epaint::Rect {
                                    min: eframe::epaint::Pos2 {
                                        x: j as f32 * scale,
                                        y: i as f32 * scale,
                                    },
                                    max: eframe::epaint::Pos2 {
                                        x: (j + 1) as f32 * scale,
                                        y: (i + 1) as f32 * scale,
                                    },
                                },
                                0.0,
                                match t {
                                    Tile::Wall => Color32::DARK_GRAY,
                                    Tile::Passage => Color32::GRAY,
                                    Tile::BeenBefore => Color32::LIGHT_BLUE,
                                },
                            );
                        }
                    }

                    ui.painter().rect_filled(
                        eframe::epaint::Rect {
                            min: Pos2 {
                                x: data.target.get_x() as f32 * scale,
                                y: data.target.get_y() as f32 * scale,
                            },
                            max: Pos2 {
                                x: (data.target.get_x() + 1) as f32 * scale,
                                y: (data.target.get_y() + 1) as f32 * scale,
                            },
                        },
                        0.0,
                        Color32::GREEN,
                    );

                    ui.painter().add(Shape::Path(PathShape {
                        points: (match data.robot_facing {
                            Heading::North => [(0.0, 1.0), (0.5, 0.0), (1.0, 1.0)],
                            Heading::East => [(0.0, 0.0), (1.0, 0.5), (0.0, 1.0)],
                            Heading::South => [(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)],
                            Heading::West => [(1.0, 0.0), (1.0, 1.0), (0.0, 0.5)],
                        })
                        .iter()
                        .map(|(x, y)| Pos2 {
                            x: (data.robot_pos.get_x() as f32 + x) * scale,
                            y: (data.robot_pos.get_y() as f32 + y) * scale,
                        })
                        .collect(),
                        closed: true,
                        fill: Color32::BLUE,
                        stroke: Stroke::default(),
                    }));
                });

                if self.active {
                    ctx.request_repaint();
                }
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Maze App");
            ui.label("Width: ");
            ui.add(egui::Slider::new(&mut self.width, 0..=120));
            ui.label("Height: ");
            ui.add(egui::Slider::new(&mut self.height, 0..=120));

            if ui.button("Generate Maze").clicked() {
                let mut gen = GappedPrimGenerator::new();
                gen.set_options(GeneratorOptions {
                    width: self.width,
                    height: self.height,
                });
                self.maze = gen.generate_maze();
                self.prev_robot_prog = Some(RobotProgress { maze: self.maze.clone(), robot_pos: Point(1, 1), robot_facing: Heading::East, target: Point(self.maze.get_width() - 2, self.maze.get_height() - 2)});
            }

            if ui.button("Start Maze").clicked() {
                match self.transmitter.send(Message {
                    maze: self.maze.clone(),
                    start: true,
                }) {
                    Err(err) => println!("{:?}", err),
                    _ => (),
                };

                self.active = true;
            }
        });
    }
}
