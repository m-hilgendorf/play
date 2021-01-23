#![allow(dead_code)]
mod audio_file;
mod audio_stream;
mod sample_player;
mod utils;

use audio_stream::audio_stream;
use sample_player::*;

fn main() {
    // get program input...
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("usage is: `play <path>`");
        std::process::exit(1);
    }

    // Create the sample player and controller
    let (mut player, mut controller) = sample_player();

    // initialize state and begin the stream...
    let _stream = audio_stream(move |mut context| {
        player.advance(&mut context);
    });

    // some random operations
    controller.load_file(&args[1]);
    controller.play();
    let duration =
        (controller.duration_samples().unwrap() as f64) / controller.sample_rate().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(duration as u64));
    controller.stop();
    controller.seek(0.0);
    controller.set_active(0, false);
    controller.play();
    std::thread::sleep(std::time::Duration::from_secs(duration as u64));
    controller.stop();
    controller.seek(0.0);
    controller.set_active(1, false);
    controller.set_active(0, true);
    controller.play();
    std::thread::sleep(std::time::Duration::from_secs(duration as u64));
}
