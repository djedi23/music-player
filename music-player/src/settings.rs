use crate::player_state::{Repeat, Shuffle};
use clap::ArgMatches;
use config::{Config, Environment, File};
use directories::{BaseDirs, ProjectDirs};
use miette::{bail, IntoDiagnostic, Result, WrapErr};
use serde::{Deserialize, Serialize};
use std::{
  fmt::{Display, Error},
  fs::{self, remove_file},
  path::{Path, PathBuf},
};
use toml::{from_str, to_string_pretty};
use tracing::{debug, instrument, trace};
use url::Url;

const QUALIFIER: &str = "org";
const ORGANISATION: &str = "djedi";
const APPLICATION: &str = "music-player";

#[derive(Debug, Deserialize)]
pub(crate) struct Settings {
  pub(crate) playlist_path: String,
}

#[instrument(skip(matches))]
pub(crate) fn settings(matches: &ArgMatches) -> Result<Settings> {
  let env_prefix: &str = "MUSIC-PLAYER-RS";
  let mut settings_builder = Config::builder();
  settings_builder = settings_builder
    .set_default("uri", "http://localhost:8080")
    .into_diagnostic()?;

  if let Some(proj_dirs) = ProjectDirs::from(QUALIFIER, ORGANISATION, APPLICATION) {
    let path = Path::new(proj_dirs.config_dir()).join("settings.toml");
    let path = path.to_str().unwrap();
    settings_builder = settings_builder.add_source(File::with_name(path).required(false));
    settings_builder = settings_builder
      .set_default("configuration_path", path)
      .into_diagnostic()?;

    debug!("Try to load config file: {}", &path);
  }
  settings_builder = if let Some(base_dir) = BaseDirs::new() {
    settings_builder.set_default(
      "playlist_path",
      base_dir
        .data_local_dir()
        .join("rhythmbox")
        .join("rhythmdb.xml")
        .display()
        .to_string(),
    )
  } else {
    settings_builder.set_default("playlist_path", "")
  }
  .into_diagnostic()?;

  settings_builder = settings_builder.add_source(Environment::with_prefix(env_prefix));
  let config = settings_builder.build().into_diagnostic()?;
  let mut settings: Settings = config.clone().try_deserialize().into_diagnostic()?;

  settings.playlist_path = get_settings(&config, matches, "playlist_path")?;

  Ok(settings)
}

#[instrument(skip(config, matches))]
fn get_settings(config: &Config, matches: &ArgMatches, arg: &str) -> Result<String> {
  if let Some(value) = matches.get_one::<String>(arg) {
    Ok(value.clone())
  } else if let Some(profile) = matches.get_one::<String>("profile") {
    trace!("profile: {profile}");
    if let Ok(value) = config.get_string(&format!("profile.{profile}.{arg}")) {
      Ok(value)
    } else if let Ok(value) = config.get_string(arg) {
      trace!("profile {profile} not found. Fallback to default profile.");
      Ok(value)
    } else {
      bail!("Setting not found")
    }
  } else if let Ok(value) = config.get_string(arg) {
    Ok(value)
  } else {
    bail!("Setting not found")
  }
}

#[derive(Debug, Deserialize, Serialize)]
pub(crate) struct PlayerStateSetting {
  pub(crate) track: Option<Url>,
  pub(crate) position: Option<u64>,
  pub(crate) shuffle_mode: Option<Shuffle>,
  pub(crate) repeat_mode: Option<Repeat>,
}

impl PlayerStateSetting {
  #[instrument]
  pub(crate) fn load() -> Result<Option<PlayerStateSetting>> {
    if let Some(path) = Self::get_path() {
      if let Ok(str) = fs::read_to_string(path) {
        return Ok(Some(from_str(&str).into_diagnostic()?));
      }
    }
    Ok(None)
  }
  #[instrument]
  pub(crate) fn save(&self) -> Result<()> {
    if let Some(path) = Self::get_path() {
      fs::write(&path, to_string_pretty(self).into_diagnostic()?.as_bytes())
        .into_diagnostic()
        .with_context(|| format!("Trying to save `{}`", &path.display()))?;
    }
    Ok(())
  }

  fn get_path() -> Option<PathBuf> {
    BaseDirs::new().map(|base_dir| {
      Path::new(base_dir.data_local_dir())
        .join("rhythmbox")
        .join("music_player.toml")
        .to_path_buf()
    })
  }

  pub(crate) fn dump() -> Result<()> {
    println!(
      "Configuration File: {}",
      PlayerStateSetting::get_path()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
    );
    if let Some(saved_track_and_position) = PlayerStateSetting::load()? {
      println!("{saved_track_and_position}");
    }
    Ok(())
  }

  pub(crate) fn clean() -> Result<()> {
    use miette::miette;
    remove_file(Self::get_path().ok_or(miette!("Can't get path"))?).into_diagnostic()
  }
}

impl Display for PlayerStateSetting {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    f.write_str(&to_string_pretty(self).map_err(|_| Error)?)
  }
}
