use super::Ui;
use crate::{
  player_state::{PlayerState, Repeat, Shuffle},
  settings::{PlayerStateSetting, Settings},
  ui::{filter_playlist, rendering::render_table, Order, OrderDir, Panel, TabSelection},
};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use miette::Result;
use std::ops::{Deref, DerefMut};
use tracing::{debug, instrument};

pub(crate) enum EventProcessStatus {
  None,
  Quit,
}

#[instrument(skip(app, player))]
pub(crate) async fn handle_keys(
  key: KeyEvent,
  app: &mut Ui<'_>,
  player: &'static PlayerState,
  settings: &Settings,
) -> Result<EventProcessStatus> {
  debug!("{:?}", key);
  if key.kind == KeyEventKind::Press {
    match (&app.panel, key.modifiers, key.code) {
      // ctrl-c, exc : Quit
      (_, KeyModifiers::CONTROL, KeyCode::Char('c')) | (_, KeyModifiers::NONE, KeyCode::Esc) => {
        if let Some(pipeline) = player.get_pipeline().await {
          use gstreamer::{prelude::ElementExt, State};

          let (_, state, _) = pipeline.state(None);
          let pstate = if state == State::Playing || state == State::Paused {
            PlayerStateSetting {
              track: player.get_track().await.as_ref().map(|x| x.get_location()),
              position: player.track_position().await.ok(),
              shuffle_mode: Some(*player.shuffle_mode.read().await),
              repeat_mode: Some(*player.repeat_mode.read().await),
            }
          } else {
            PlayerStateSetting {
              track: None,
              position: None,
              repeat_mode: None,
              shuffle_mode: None,
            }
          };
          pstate.save()?;
        }
        player.get_queue().await.save()?;
        return Ok(EventProcessStatus::Quit);
      }
      // enter: play the selected track
      (Panel::None, KeyModifiers::NONE, KeyCode::Enter) => {
        let track_list = player.get_playlist().await;
        let track = track_list[app.table_state.selected().unwrap_or_default()].clone();
        player.stop_track().await?;
        player.play_track(track).await?;
      }
      // down: select the next track
      (Panel::None, KeyModifiers::NONE, KeyCode::Down) => {
        let i = match app.table_state.selected() {
          Some(i) => {
            if i >= app.row_len - 1 {
              0
            } else {
              i + 1
            }
          }
          None => 0,
        };
        app.table_state.select(Some(i));
      }
      // home: select the fist track
      (Panel::None, KeyModifiers::NONE, KeyCode::Home) => {
        app.table_state.select(Some(0));
      }
      // up: select the previous track
      (Panel::None, KeyModifiers::NONE, KeyCode::Up) => {
        let i = match app.table_state.selected() {
          Some(i) => {
            if i == 0 {
              app.row_len - 1
            } else {
              i - 1
            }
          }
          None => 0,
        };
        app.table_state.select(Some(i));
      }
      // page down:
      (Panel::None, KeyModifiers::NONE, KeyCode::PageDown) => {
        let i = match app.table_state.selected() {
          Some(i) => {
            if i >= app.row_len - 15 {
              0
            } else {
              i + 15 // FIXME: height on the rect
            }
          }
          None => 0,
        };
        app.table_state.select(Some(i));
      }
      // page up
      (Panel::None, KeyModifiers::NONE, KeyCode::PageUp) => {
        let i = match app.table_state.selected() {
          Some(i) => {
            if i < 15 {
              app.row_len - 1
            } else {
              i - 15
            }
          }
          None => 0,
        };
        app.table_state.select(Some(i));
      }

      // <-- : seek 5 secs before
      (Panel::None, KeyModifiers::NONE, KeyCode::Left) => {
        if let Some(pipeline) = player.get_pipeline().await {
          let position = app.get_track_elapsed_duration(&pipeline);
          let new_position: i64 = position.as_secs() as i64 - 5;
          let new_position = if new_position < 0 {
            0
          } else {
            new_position as u64
          };
          player.track_seek(new_position).await?;
        }
      }
      // --> : seek 5 secs after
      (Panel::None, KeyModifiers::NONE, KeyCode::Right) => {
        if let Some(pipeline) = player.get_pipeline().await {
          let position = app.get_track_elapsed_duration(&pipeline);
          player.track_seek(5 + position.as_secs()).await?;
        }
      }
      // alt-g : go to the track played in the current view
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('g')) => {
        if let Some(track) = &*player.get_track().await {
          if let Some(index) = player.find_track_index(track).await {
            app.table_state.select(Some(index));
          }
        }
      }
      // alt-p : view podcasts
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('p')) => {
        app.selected_tab = TabSelection::Podcast;
        build_table(app, player, true).await;
      }
      // alt-m: view musics
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('m')) => {
        app.selected_tab = TabSelection::Music;
        build_table(app, player, true).await;
      }
      // alt-q: view queue
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('q')) => {
        app.selected_tab = TabSelection::Queue;
        build_table(app, player, true).await;
      }

      // alt-e: enqueue
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('e')) => {
        if app.selected_tab != TabSelection::Queue {
          if let Some(index) = app.table_state.selected() {
            let track_list = player.get_playlist().await;
            let track = &track_list[index];
            player.queue.write().await.enqueue(track.get_location());
          };
        }
      }

      // alt-o: shuffle mode
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('o')) => {
        player
          .set_shuffle_mode(match player.get_shuffle_mode().await {
            Shuffle::Next => Shuffle::Shuffle,
            Shuffle::Shuffle => Shuffle::ShuffleLastPlayed,
            Shuffle::ShuffleLastPlayed => Shuffle::Next,
          })
          .await;
      }

      // alt-c: repeat current track
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('c')) => {
        player
          .set_repeat_mode(match player.get_repeat_mode().await {
            Repeat::AllTracks => Repeat::CurrentTrack,
            Repeat::CurrentTrack => Repeat::AllTracks,
          })
          .await
      }

      // alt-h: display help
      (_, KeyModifiers::ALT, KeyCode::Char('h')) => {
        app.panel = match app.panel {
          Panel::None => Panel::Help,
          Panel::Help => Panel::None,
        }
      }

      // ////////////////////////////////////////
      // Order
      // ////////////////////////////////////////

      // alt-s: order-by score/default
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('s')) => {
        order_column(app, player, Order::Default).await;
      }

      // alt-t: order-by title
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('t')) => {
        order_column(app, player, Order::Title).await;
      }

      // alt-d: order-by date
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('d')) => {
        order_column(app, player, Order::Date).await;
      }
      // alt-r: order-by rating
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('r')) => {
        order_column(app, player, Order::Rating).await;
      }

      // alt-l: order-by last played
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('l')) => {
        order_column(app, player, Order::LastPlayed).await;
      }

      // ////////////////////////////////////////
      // Raring
      // ////////////////////////////////////////
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('0')) => {
        player
          .update_rating(
            player.get_mut_db().await.deref_mut(),
            app.table_state.selected(),
            0,
            settings,
          )
          .await?;
        build_table(app, player, false).await;
      }
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('1')) => {
        player
          .update_rating(
            player.get_mut_db().await.deref_mut(),
            app.table_state.selected(),
            1,
            settings,
          )
          .await?;
        build_table(app, player, false).await;
      }
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('2')) => {
        player
          .update_rating(
            player.get_mut_db().await.deref_mut(),
            app.table_state.selected(),
            2,
            settings,
          )
          .await?;
        build_table(app, player, false).await;
      }
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('3')) => {
        player
          .update_rating(
            player.get_mut_db().await.deref_mut(),
            app.table_state.selected(),
            3,
            settings,
          )
          .await?;
        build_table(app, player, false).await;
      }
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('4')) => {
        player
          .update_rating(
            player.get_mut_db().await.deref_mut(),
            app.table_state.selected(),
            4,
            settings,
          )
          .await?;
        build_table(app, player, false).await;
      }
      (Panel::None, KeyModifiers::ALT, KeyCode::Char('5')) => {
        player
          .update_rating(
            player.get_mut_db().await.deref_mut(),
            app.table_state.selected(),
            5,
            settings,
          )
          .await?;
        build_table(app, player, false).await;
      }

      // ////////////////////////////////////////
      // Search
      // ////////////////////////////////////////

      // backspace: delete previous char in search
      (Panel::None, KeyModifiers::NONE, KeyCode::Backspace) => {
        app.search.pop();
        build_table(app, player, true).await;
      }
      (Panel::None, KeyModifiers::NONE, KeyCode::Char(c)) => {
        app.search = app.search.clone() + &c.to_string();
        app.order_by = Order::Default;
        app.order_dir = OrderDir::Desc;
        build_table(app, player, true).await;
      }
      _ => {}
    }
  }

  Ok(EventProcessStatus::None)
}

#[instrument(skip(app, player))]
async fn order_column(app: &mut Ui<'_>, player: &'static PlayerState, column: Order) {
  if app.order_by == column {
    if app.order_dir == OrderDir::Asc {
      app.order_dir = OrderDir::Desc;
    } else {
      app.order_dir = OrderDir::Asc;
    }
  } else {
    app.order_by = column;
    app.order_dir = OrderDir::Desc;
  }
  build_table(app, player, true).await;
}

#[instrument(skip(app, player))]
pub(crate) async fn build_table(app: &mut Ui<'_>, player: &'static PlayerState, set_select: bool) {
  let track_list = filter_playlist(
    app.selected_tab,
    &app.search,
    player.get_db().await.deref(),
    player.get_queue().await.deref(),
    app.order_by,
    app.order_dir,
  );

  let (rows_len, table, track_index) = render_table(
    &track_list,
    app.order_by,
    app.order_dir,
    &*player.get_track().await,
    app.selected_tab,
  );
  player.set_playlist(track_list).await;
  app.table = table;
  app.row_len = rows_len;
  if set_select {
    app.table_state.select(None);
    use crate::player_state::UiNotification;
    let _ = player
      .notify_ui(UiNotification::UpdateIndex(track_index))
      .await;
  }
}
