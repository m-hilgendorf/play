mod audio_file;
mod audio_stream;
mod sample_player;
mod utils;

use audio_stream::audio_stream;
use basedrop::Collector;
use sample_player::*;

fn main() {
    // get program input...
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 {
        println!("usage is: `play <path>`");
        std::process::exit(1);
    }
    // initialize gc
    let mut gc = Collector::new();

    // Create the sample player and controller
    let (mut player, mut controller) = sample_player(&gc);

    // initialize state and begin the stream...
    let _stream = audio_stream(move |mut context| {
        player.advance(&mut context);
    });

    // some random operations
    controller.load_file(&args[1]);
    controller.play();
    std::thread::sleep(std::time::Duration::from_millis(500));
    controller.stop();
    controller.seek(0.0);
    controller.set_active(0, false);
    controller.play();
    std::thread::sleep(std::time::Duration::from_millis(500));
    controller.stop();
    controller.seek(0.0);
    controller.set_active(1, false);
    controller.set_active(0, true);
    controller.play();
    std::thread::sleep(std::time::Duration::from_millis(500));
    controller.stop();
    std::thread::sleep(std::time::Duration::from_millis(500));
    controller.set_active(0, true);
    controller.set_active(1, true);
    controller.play();
    std::thread::sleep(std::time::Duration::from_millis(3000));
    gc.collect();
}
