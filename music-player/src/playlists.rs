use directories::BaseDirs;
use miette::{Context, IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::{
  fs,
  path::{Path, PathBuf},
};
use toml::{from_str, to_string_pretty};
use tracing::instrument;
use url::Url;
// uick_xml::impl_deserialize_for_internally_tagged_enum;

#[derive(Serialize, Deserialize, Debug)]
#[serde(rename = "rhythmdb-playlists")]
pub(crate) struct RhythmdbPlaylists {
  playlist: Vec<Playlist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase", tag = "type")]
pub(crate) enum Playlist {
  Automatic(AutomaticPlaylist),
  Static(StaticPlaylist),
  Queue(QueuePlaylist),
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct AutomaticPlaylist {
  #[serde(rename = "@name")]
  name: String,
  #[serde(rename = "@show-browser")]
  show_browser: String,
  #[serde(rename = "@browser-position")]
  browser_position: u64,
  #[serde(rename = "@search-type")]
  search_type: String,
  #[serde(rename = "@sort-key")]
  sort_key: String,
  #[serde(rename = "@sort-direction")]
  sort_direction: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct StaticPlaylist {
  #[serde(rename = "@name")]
  name: String,
  #[serde(rename = "@show-browser")]
  show_browser: String,
  #[serde(rename = "@browser-position")]
  browser_position: u64,
  #[serde(rename = "@search-type")]
  search_type: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub(crate) struct QueuePlaylist {
  #[serde(rename = "@name")]
  name: String,
  #[serde(rename = "@show-browser")]
  show_browser: bool,
  #[serde(rename = "@browser-position")]
  browser_position: u64,
  #[serde(rename = "@search-type")]
  search_type: String,
  pub(crate) location: Vec<Url>,
}

impl Playlist {
  pub(crate) fn new() -> Playlist {
    Playlist::Queue(QueuePlaylist {
      name: "Queue".into(),
      show_browser: false,
      browser_position: 180,
      search_type: "search-match".into(),
      location: vec![],
    })
  }

  fn get_path() -> Option<PathBuf> {
    BaseDirs::new().map(|base_dir| {
      Path::new(base_dir.data_local_dir())
        .join("rhythmbox")
        .join("playlist.toml")
        .to_path_buf()
    })
  }

  #[instrument]
  pub(crate) fn load() -> Result<Playlist> {
    if let Some(path) = Self::get_path() {
      if let Ok(str) = fs::read_to_string(path) {
        return from_str(&str).into_diagnostic();
      }
    }
    Ok(Playlist::new())
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

  pub(crate) fn dump() -> Result<()> {
    println!(
      "Configuration File: {}",
      Playlist::get_path()
        .unwrap_or_default()
        .to_str()
        .unwrap_or_default()
    );
    let playlist = Playlist::load()?;
    println!("{}", to_string_pretty(&playlist).into_diagnostic()?);

    Ok(())
  }

  pub(crate) fn clean() -> Result<()> {
    use miette::miette;
    use std::fs::remove_file;
    remove_file(Self::get_path().ok_or(miette!("Can't get path"))?).into_diagnostic()
  }

  #[instrument]
  pub(crate) fn enqueue(&mut self, track: Url) {
    match self {
      Playlist::Queue(queue) => queue.location.push(track),
      _ => unimplemented!(),
    }
  }

  #[instrument]
  pub(crate) fn remove(&mut self, track: Url) {
    match self {
      Playlist::Queue(queue) => {
        queue.location.retain(|url| *url != track);
      }
      _ => unimplemented!(),
    }
  }

  #[instrument]
  pub(crate) fn queue(&self) -> Vec<Url> {
    match self {
      Playlist::Queue(queue) => queue.location.clone(),
      _ => unimplemented!(),
    }
  }
}
