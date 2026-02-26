use argh::FromArgs;
use std::{error, path::PathBuf};
use willhook::{InputEvent, IsSystemKeyPress::Normal, KeyPress, keyboard_hook};

mod parser;
use parser::Config;

mod models;

#[derive(FromArgs)]
/// Vertaxio, aim as good as masterchief, bruv...
struct Args {
    /// path to config file. (default lamine.yml)
    #[argh(option, short = 'c')]
    config: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn error::Error>> {
    let args: Args = argh::from_env();
    let cfg: Config = parser::Config::load(args.config)?;

    let rx = keyboard_hook().unwrap();

    loop {
        match rx.try_recv() {
            Ok(InputEvent::Keyboard(ke)) if ke.pressed == KeyPress::Down(Normal) => {
                if ke.key == Some(cfg.exit_key) {
                    break;
                }
            }
            Ok(_) => {}
            Err(_) => std::thread::sleep(std::time::Duration::from_millis(10)),
        }
    }
    Ok(())
}
