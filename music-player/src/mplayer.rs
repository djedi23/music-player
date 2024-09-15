use crate::{
  gstreamer::{pause, play},
  player_state::PlayerState,
};
use mpris_server::{
  zbus::fdo, LoopStatus, Metadata, PlaybackStatus, PlayerInterface, RootInterface, Time, Volume,
};
use tracing::{info, instrument, warn};

impl RootInterface for PlayerState {
  #[instrument(skip(self))]
  async fn identity(&self) -> fdo::Result<String> {
    Ok("music-player".into())
  }

  #[instrument(skip(self))]
  async fn raise(&self) -> fdo::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn quit(&self) -> fdo::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn can_quit(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  #[instrument(skip(self))]
  async fn fullscreen(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  #[instrument(skip(self))]
  async fn set_fullscreen(&self, _fullscreen: bool) -> mpris_server::zbus::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  #[instrument(skip(self))]
  async fn can_raise(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  #[instrument(skip(self))]
  async fn has_track_list(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  #[instrument(skip(self))]
  async fn desktop_entry(&self) -> fdo::Result<String> {
    Ok("".into())
  }

  #[instrument(skip(self))]
  async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
    Ok(vec![])
  }

  #[instrument(skip(self))]
  async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
    Ok(vec![])
  }
}

impl PlayerInterface for PlayerState {
  #[instrument(skip(self))]
  async fn set_volume(&self, _volume: Volume) -> mpris_server::zbus::Result<()> {
    Ok(())
  }

  #[instrument(skip(self), ret)]
  #[instrument(skip(self))]
  async fn metadata(&self) -> fdo::Result<Metadata> {
    if let Some(track) = &*self.get_track().await {
      info!("Metadata {:?}", &track);
      Ok((&**track).into())
    } else {
      info!("Metadata None");
      let mut metadata = Metadata::default();
      metadata.set_title(Some("No Song"));
      Ok(metadata)
    }
  }

  #[instrument(skip(self))]
  async fn next(&self) -> fdo::Result<()> {
    self
      .next_track()
      .await
      .map_err(|e| fdo::Error::Failed(e.to_string()))?;
    Ok(())
  }

  #[instrument(skip(self))]
  async fn previous(&self) -> fdo::Result<()> {
    warn!("Not implemented and silently do nothing.");
    Ok(())
  }

  #[instrument(skip(self))]
  async fn pause(&self) -> fdo::Result<()> {
    let current_pipeline = self.get_pipeline().await;
    if let Some(pipeline) = current_pipeline {
      pause(&pipeline).map_err(|e| fdo::Error::Failed(e.to_string()))?;
    }

    Ok(())
  }

  #[instrument(skip(self))]
  async fn play_pause(&self) -> fdo::Result<()> {
    use gstreamer::{prelude::ElementExt, State};
    let current_pipeline = self.get_pipeline().await;
    if let Some(pipeline) = current_pipeline {
      let (_, state, _) = pipeline.state(None);
      if state == State::Playing {
        pause(&pipeline).map_err(|e| fdo::Error::Failed(e.to_string()))?;
      } else {
        play(&pipeline).map_err(|e| fdo::Error::Failed(e.to_string()))?;
      }
    }

    Ok(())
  }

  #[instrument(skip(self))]
  async fn stop(&self) -> fdo::Result<()> {
    self
      .stop_track()
      .await
      .map_err(|e| fdo::Error::Failed(e.to_string()))?;

    Ok(())
  }

  #[instrument(skip(self))]
  async fn play(&self) -> fdo::Result<()> {
    let current_pipeline = self.get_pipeline().await;
    if let Some(pipeline) = current_pipeline {
      play(&pipeline).map_err(|e| fdo::Error::Failed(e.to_string()))?;
    }

    Ok(())
  }

  #[instrument(skip(self))]
  async fn seek(&self, _offset: Time) -> fdo::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn set_position(&self, _track_id: mpris_server::TrackId, _position: Time) -> fdo::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn playback_status(&self) -> fdo::Result<mpris_server::PlaybackStatus> {
    use gstreamer::{prelude::ElementExt, State};
    let current_pipeline = self.get_pipeline().await;
    Ok(if let Some(pipeline) = current_pipeline {
      let (_, state, _) = pipeline.state(None);
      match state {
        State::VoidPending | State::Null | State::Ready => PlaybackStatus::Stopped,
        State::Paused => PlaybackStatus::Paused,
        State::Playing => PlaybackStatus::Playing,
      }
    } else {
      PlaybackStatus::Stopped
    })
  }

  #[instrument(skip(self))]
  async fn loop_status(&self) -> fdo::Result<mpris_server::LoopStatus> {
    Ok(LoopStatus::None)
  }

  #[instrument(skip(self))]
  async fn set_loop_status(
    &self,
    _loop_status: mpris_server::LoopStatus,
  ) -> mpris_server::zbus::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
    Ok(1.0)
  }

  #[instrument(skip(self))]
  async fn set_rate(&self, _rate: mpris_server::PlaybackRate) -> mpris_server::zbus::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn shuffle(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  #[instrument(skip(self))]
  async fn set_shuffle(&self, _shuffle: bool) -> mpris_server::zbus::Result<()> {
    todo!()
  }

  #[instrument(skip(self))]
  async fn volume(&self) -> fdo::Result<Volume> {
    Ok(1.0)
  }

  #[instrument(skip(self))]
  async fn position(&self) -> fdo::Result<Time> {
    Ok(Time::from_millis(
      self
        .track_position()
        .await
        .map_err(|e| fdo::Error::Failed(e.to_string()))? as i64,
    ))
  }

  #[instrument(skip(self))]
  async fn minimum_rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
    Ok(0.5)
  }

  #[instrument(skip(self))]
  async fn maximum_rate(&self) -> fdo::Result<mpris_server::PlaybackRate> {
    Ok(1.5)
  }

  #[instrument(skip(self))]
  async fn can_go_next(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  #[instrument(skip(self))]
  async fn can_go_previous(&self) -> fdo::Result<bool> {
    Ok(false)
  }

  #[instrument(skip(self))]
  async fn can_play(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  #[instrument(skip(self))]
  async fn can_pause(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  #[instrument(skip(self))]
  async fn can_seek(&self) -> fdo::Result<bool> {
    Ok(true)
  }

  #[instrument(skip(self))]
  async fn can_control(&self) -> fdo::Result<bool> {
    Ok(true)
  }
}
