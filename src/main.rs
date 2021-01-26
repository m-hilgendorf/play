#![allow(dead_code)]
mod audio_file;
mod audio_stream;
mod sample_player;
mod ui;
mod utils;
use audio_stream::audio_stream;
use basedrop::Collector;
use sample_player::*;

fn main() -> Result<(), druid::PlatformError> {
    // get program input...
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("usage is: `play <path>`");
        std::process::exit(1);
    }
    // initialize gc
    let gc = Collector::new();

    // Create the sample player and controller
    let (mut player, mut controller) = sample_player(&gc);

    // initialize state and begin the stream...
    let _stream = audio_stream(move |mut context| {
        player.advance(&mut context);
    });
    controller.load_file(&args[1]);
    ui::run(gc, controller)
}
