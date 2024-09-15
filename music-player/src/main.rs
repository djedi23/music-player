mod args;
mod gstreamer;
mod mplayer;
mod player_state;
mod playlists;
mod rhythmdb;
mod settings;
mod trace;
mod ui;

use crate::{
  args::{gen_completions, App, Commands},
  gstreamer::{gstreamer_init, start_playing},
  player_state::PlayerState,
  rhythmdb::Rhythmdb,
};
use args::Config;
use clap::{CommandFactory, Parser};
use if_chain::if_chain;
use miette::{miette, IntoDiagnostic, Result};
use mpris_server::Server;
use playlists::Playlist;
use rhythmdb::{Entry, SongEntry};
use settings::{settings, PlayerStateSetting};
use std::sync::Arc;
use tokio::sync::OnceCell;
use trace::init_tracing;
use url::Url;

// One singletton to rule them all!
static MPRIS: OnceCell<Server<PlayerState>> = OnceCell::const_new();

pub(crate) async fn get_mpris_server() -> Result<&'static Server<PlayerState>> {
  MPRIS
    .get_or_try_init(|| async {
      let mpris_server_data = PlayerState::new();
      Server::new("org.djedi.music-player", mpris_server_data)
        .await
        .into_diagnostic()
    })
    .await
}

#[tokio::main]
async fn main() -> Result<()> {
  init_tracing()?;
  let args = App::parse();
  let config = settings(&App::command().get_matches())?;
  gen_completions(&args);

  if let Some(Commands::Config(c)) = &args.command {
    match c {
      Config::Show => {
        PlayerStateSetting::dump()?;
        Playlist::dump()?;
        std::process::exit(0);
      }
      Config::Clean(args) => {
        if args.main {
          PlayerStateSetting::clean()?;
        }
        if args.playlist {
          Playlist::clean()?;
        }
        if args.ignored_entries {
          Rhythmdb::clean_ignored_entries(&config)?;
        }
        std::process::exit(0);
      }
      Config::ShowIgnoredEntries => {
        Rhythmdb::show_ignored_entries(&config)?;
        std::process::exit(0);
      }
    }
  }

  let db = Rhythmdb::load(&config)?;

  // Init the app component: gstreamer and mpris protocol
  gstreamer_init()?;
  let mpris_server = get_mpris_server().await?;
  let player_app = mpris_server.imp();

  if let Ok(q) = Playlist::load() {
    player_app.set_queue(q).await;
  }

  // Try to init shuffle and repeat mode from saved state file.
  if let Some(saved_track_and_position) = PlayerStateSetting::load()? {
    if let Some(shuffle) = saved_track_and_position.shuffle_mode {
      player_app.set_shuffle_mode(shuffle).await;
    }
    if let Some(repeat) = saved_track_and_position.repeat_mode {
      player_app.set_repeat_mode(repeat).await;
    }
  }

  // Find the track to play on startup
  let mut start_index = 0;
  let track_list = db.filter_by_song("", ui::Order::Default, ui::OrderDir::Desc);
  // Play the track from the cli args
  if let Some(file) = args.file {
    let mut track = if let Ok(tag) = id3::Tag::read_from_path(&file) {
      SongEntry::from(tag)
    } else {
      SongEntry::default()
    };
    track.location =
      Url::from_file_path(&file).map_err(|_| miette!("Can't parse file path: '{file}'"))?;
    player_app.play_track(Arc::new(Entry::Song(track))).await?;
  } else if !track_list.is_empty() {
    // Try to play the saved file or a random one.
    start_index = player_saved_track(player_app, &db, &track_list).await?;
  }

  player_app.set_db(db).await;

  ui::ui(start_index, &config).await?;
  Ok(())
}

async fn play_saved_file(
  player_app: &PlayerState,
  saved_track_and_position: &PlayerStateSetting,
  track_list: &[Arc<Entry>],
  track: Arc<Entry>,
) -> Result<usize> {
  player_app.set_playlist(track_list.to_vec()).await;
  let start_index: usize = player_app.find_track_index(&track).await.unwrap_or(0);
  player_app.play_track(track).await?;
  if let Some(position) = saved_track_and_position.position {
    if let Some(pipeline) = player_app.get_pipeline().await {
      use ::gstreamer::{prelude::ElementExt, State};
      let (_, state, _) = pipeline.state(None);
      if state == State::Playing || state == State::Paused {
        player_app.track_seek(position / 1000).await?;
      }
    }
  }
  Ok(start_index)
}

#[rustfmt::skip::macros(if_chain)]
async fn player_saved_track(
  player_app: &PlayerState,
  db: &Rhythmdb,
  track_list: &[Arc<Entry>],
) -> Result<usize> {
  let mut start_index = 0;
  if_chain! {
      if let Some(saved_track_and_position) = PlayerStateSetting::load()?;
      if let Some(ref url) = saved_track_and_position.track;
      if let Some(track) = db.find_url(url);
      then {
          start_index= play_saved_file(player_app, &saved_track_and_position, track_list, track).await?;
      }else {
	  let (track,_)= PlayerState::choose_track(track_list)?;
	  player_app.play_track(track).await?;
          player_app.set_playlist(track_list.to_vec()).await;
      }
  }
  Ok(start_index)
}
