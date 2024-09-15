mod events;
mod help;
mod rendering;

use self::{
  events::{build_table, handle_keys, EventProcessStatus},
  rendering::render_table,
};
use crate::{
  get_mpris_server,
  player_state::{PlayerState, UiNotification},
  playlists::Playlist,
  rhythmdb::{Entry, EntryList},
  settings::Settings,
  ui::rendering::render_ui,
  Rhythmdb,
};
use crossterm::event::{self};
use futures::{FutureExt, StreamExt};
use gstreamer::{Element, MessageView};
use if_chain::if_chain;
use miette::{IntoDiagnostic, Result};
use ratatui::widgets::{Table, TableState};
use std::{sync::Arc, time::Duration};
use tokio::{select, sync::mpsc::channel};
use tracing::{instrument, trace};

#[derive(Copy, Clone, Debug, PartialEq)]
enum TabSelection {
  Music = 0,
  Podcast = 1,
  Queue = 2,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Order {
  Default,
  Title,
  Date,
  Rating,
  LastPlayed,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum OrderDir {
  Asc,
  Desc,
}

#[derive(PartialEq, Debug)]
pub(crate) enum Panel {
  Help,
  None,
}

struct Ui<'a> {
  selected_tab: TabSelection,
  panel: Panel,
  // Sometime the track position is none so we will use this
  current_elapsed_duration: Duration,
  table_state: TableState,
  table: Table<'a>,
  row_len: usize,
  search: String,
  order_by: Order,
  order_dir: OrderDir,
}

impl<'a> Ui<'a> {
  fn new(start_index: usize) -> Ui<'a> {
    let mut result = Ui {
      selected_tab: TabSelection::Music,
      panel: Panel::None,
      current_elapsed_duration: Duration::from_secs(0),
      table_state: TableState::default(),
      table: Table::default(),
      row_len: 0,
      search: "".into(),
      order_by: Order::Default,
      order_dir: OrderDir::Desc,
    };
    result.table_state.select(Some(start_index));
    result
  }

  #[instrument(skip(self))]
  fn get_track_elapsed_duration(&mut self, pipeline: &Element) -> Duration {
    use gstreamer::{prelude::ElementExtManual, ClockTime};
    if let Some(position) = pipeline.query_position::<ClockTime>() {
      self.current_elapsed_duration = Duration::from_secs(position.seconds());
    }
    self.current_elapsed_duration
  }
}

#[rustfmt::skip::macros(select)]
pub(crate) async fn ui(start_index: usize, settings: &Settings) -> Result<()> {
  let player_app = get_mpris_server().await?;
  let player = player_app.imp();
  let (tx, mut rx) = channel(16);
  player.set_sender(tx).await;

  let mut app = Ui::new(start_index);
  let (rows_len, table, _) = render_table(
    &player.get_playlist().await,
    app.order_by,
    app.order_dir,
    &None,
    app.selected_tab,
  );
  app.table = table;
  app.row_len = rows_len;

  let mut terminal = ratatui::init();
  terminal.clear().into_diagnostic()?;

  let mut ct_reader = crossterm::event::EventStream::new();
  let mut tick = tokio::time::interval(Duration::from_millis(1000));

  loop {
    //  draw the UI
    if let Some(pipeline) = player.get_pipeline().await {
      if let Some(song_entry) = &*player.get_track().await {
        let shuffle_mode = player.get_shuffle_mode().await;
        let repeat_mode = player.get_repeat_mode().await;
        terminal
          .draw(|frame| {
            render_ui(
              frame,
              &mut app,
              &pipeline,
              song_entry,
              shuffle_mode,
              repeat_mode,
            )
            .expect("Error during ui rendering")
          })
          .into_diagnostic()?;
      }

      // handle events
      let crossterm_event = ct_reader.next().fuse();
      let tick_delay = tick.tick();

      use gstreamer::prelude::ElementExt;
      let gstreamer_bus = pipeline.bus();
      let evt = gstreamer_bus.unwrap();
      let mut stream = evt.stream();
      let g_event = stream.next();

      async fn go_next(player: &PlayerState, settings: &Settings) -> Result<()> {
        update_last_played(player, settings).await?;
        player.next_track().await?;
        Ok(())
      }

      select! {
	  _ = tick_delay => {
	      use gstreamer::{prelude::ElementExtManual, ClockTime};
	      // Sometime gstreamer stucks fraction of second before
	      // the end of a track and don't send EOS message. The
	      // following code is my attempt to catch the end of
	      // the track and go to the next one.
	      if_chain! {
		  if let Some(position) = pipeline.query_position::<ClockTime>();
		  if let Some (duration) = pipeline.query_duration::<ClockTime>();
		  let _ = trace!("{position:?}/{duration:?}");
		  let diff = duration.saturating_sub(position);
		  if  diff <= ClockTime::from_mseconds(100);
		  then {
		      go_next(player, settings).await?;
		  }
	      }
	  }
	  Some(msg)= g_event => {
	      trace!("{msg:?}");
	      trace!("{:?}",msg.view());
	      if let MessageView::Eos(_) = msg.view() {
		  go_next(player, settings).await?;
	      }
	  }
	  Some(Ok(evt)) = crossterm_event => {
	      if let event::Event::Key(key) = evt  {
		  if let EventProcessStatus::Quit = handle_keys(key, &mut app, player, settings).await? {
		      break;
		  }
	      }
	  }
	  Some(message) = rx.recv() => {
	      match message {
		  UiNotification::UpdateIndex(index) => app.table_state.select(index),
		  UiNotification::Position(position) => app.current_elapsed_duration = position,
		  UiNotification::RebuildTable => build_table(&mut app, player, true).await,
	      }
	  }
      }
    }
  }

  ratatui::restore();
  Ok(())
}

#[instrument(skip(player))]
async fn update_last_played(player: &PlayerState, settings: &Settings) -> Result<()> {
  if let Some(track) = &*player.get_track().await {
    let updated_track = match track.as_ref() {
      Entry::Song(song) => {
        let mut song_copy = song.to_owned();
        song_copy.last_played = Some(chrono::Local::now().timestamp() as u64);
        song_copy.play_count = match song_copy.play_count {
          Some(count) => Some(count + 1),
          None => Some(1),
        };
        Arc::new(Entry::Song(song_copy))
      }
      Entry::PodcastPost(podcast) => {
        let mut podcast_copy = podcast.to_owned();
        podcast_copy.last_played = Some(chrono::Local::now().timestamp() as u64);
        podcast_copy.play_count = match podcast_copy.play_count {
          Some(count) => Some(count + 1),
          None => Some(1),
        };
        Arc::new(Entry::PodcastPost(podcast_copy))
      }
      _ => unimplemented!(),
    };
    let mut db = player.get_mut_db().await;
    db.update_entry(updated_track);
    db.save(settings)?;
  }
  Ok(())
}

#[instrument(skip(selected_tab, db, playlist))]
fn filter_playlist(
  selected_tab: TabSelection,
  search: &str,
  db: &Rhythmdb,
  playlist: &Playlist,
  order_by: Order,
  order_dir: OrderDir,
) -> EntryList {
  match selected_tab {
    TabSelection::Music => db.filter_by_song(search, order_by, order_dir),
    TabSelection::Podcast => db.filter_by_podcast(search, order_by, order_dir),
    TabSelection::Queue => db.to_entries(playlist),
  }
}
