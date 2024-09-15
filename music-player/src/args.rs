use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Subcommand)]
pub(crate) enum Commands {
  /// Config related commands
  #[command(subcommand)]
  Config(Config),
}

#[derive(Subcommand)]
pub(crate) enum Config {
  /// Clean the configuration files when something goes wrong
  Clean(ConfigClean),
  /// Show the configuration files
  Show,
  /// Show ignored entries in DB
  ShowIgnoredEntries,
}

#[derive(Parser, Debug)]
pub(crate) struct ConfigClean {
  /// Remove the playlist file.
  /// Some obsolete podcast entries may remains in the playlist after an update.
  #[arg(long, required_unless_present_any(["main","ignored_entries"]))]
  pub(crate) playlist: bool,
  /// Delete the main config file. It contains the current track and the current play position.
  #[arg(long, required_unless_present_any(["playlist","ignored_entries"]))]
  pub(crate) main: bool,
  #[arg(long, required_unless_present_any(["main","playlist"]))]
  pub(crate) ignored_entries: bool,
}

#[derive(Parser)]
#[command(version, about, long_about = None)]
pub(crate) struct App {
  /// File to play
  pub(crate) file: Option<String>,

  /// Profile name
  #[arg(long, short)]
  profile: Option<String>,

  /// Path to the playlist
  #[arg(long)]
  playlist_path: Option<String>,

  /// Generate shell completions
  #[arg(long, value_enum)]
  completion: Option<Shell>,

  #[command(subcommand)]
  pub(crate) command: Option<Commands>,
}

pub(crate) fn gen_completions(args: &App) {
  if let Some(generator) = args.completion {
    use clap::{Command, CommandFactory};
    use clap_complete::{generate, Generator};
    use std::io;
    fn print_completions<G: Generator>(gen: G, cmd: &mut Command) {
      generate(gen, cmd, cmd.get_name().to_string(), &mut io::stdout());
    }
    let mut cmd = App::command();
    eprintln!("Generating completion file for {generator:?}...");
    print_completions(generator, &mut cmd);
    std::process::exit(0);
  }
}
