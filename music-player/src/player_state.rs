use crate::{
  get_mpris_server,
  gstreamer::stop,
  playlists::Playlist,
  rhythmdb::{Entry, EntryList, Rhythmdb, SharedEntry, SongEntry},
  start_playing,
};
use gstreamer::Element;
use miette::{IntoDiagnostic, Result};
use mpris_server::{Metadata, Property, Time};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, thread::sleep, time::Duration};
use tokio::sync::{mpsc::Sender, RwLock};
use tracing::instrument;

pub(crate) enum UiNotification {
  UpdateIndex(Option<usize>),
  Position(Duration),
  RebuildTable,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub(crate) enum Shuffle {
  Next,
  #[allow(clippy::enum_variant_names)]
  Shuffle,
  #[allow(clippy::enum_variant_names)]
  ShuffleLastPlayed,
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub(crate) enum Repeat {
  AllTracks,
  CurrentTrack,
}

//#[derive(Clone)]
pub struct PlayerState {
  pub current_track: RwLock<Option<SharedEntry>>,
  pub current_pipeline: RwLock<Option<Element>>,
  pub playlist: RwLock<EntryList>,
  pub queue: RwLock<Playlist>,
  pub db: RwLock<Rhythmdb>,
  pub sender: RwLock<Option<Sender<UiNotification>>>,
  pub shuffle_mode: RwLock<Shuffle>,
  pub repeat_mode: RwLock<Repeat>,
}

impl PlayerState {
  #[instrument]
  pub(crate) fn new() -> PlayerState {
    PlayerState {
      current_track: RwLock::new(None),
      current_pipeline: RwLock::new(None),
      playlist: RwLock::new(vec![]),
      queue: RwLock::new(Playlist::new()),
      db: RwLock::new(Rhythmdb::new()),
      sender: RwLock::new(None),
      shuffle_mode: RwLock::new(Shuffle::ShuffleLastPlayed),
      repeat_mode: RwLock::new(Repeat::AllTracks),
    }
  }

  #[instrument(skip(self))]
  pub(crate) async fn get_track(&self) -> impl std::ops::Deref<Target = Option<SharedEntry>> + '_ {
    self.current_track.read().await
  }

  #[instrument(skip(self))]
  pub(crate) async fn set_track(&self, song: SharedEntry) {
    let mut current_track = self.current_track.write().await;
    *current_track = Some(song);
  }

  #[instrument(skip(self))]
  pub(crate) async fn get_pipeline(&self) -> Option<Element> {
    let current_pipeline = self.current_pipeline.read().await;
    current_pipeline.to_owned()
  }

  #[instrument(skip(self))]
  pub(crate) async fn set_pipeline(&self, pipeline: Element) {
    let mut current_pipeline = self.current_pipeline.write().await;
    *current_pipeline = Some(pipeline);
  }

  #[instrument(skip(self))]
  pub(crate) async fn get_playlist(&self) -> impl std::ops::Deref<Target = EntryList> + '_ {
    self.playlist.read().await
  }

  #[instrument(skip(self))]
  pub(crate) async fn set_playlist(&self, p: EntryList) {
    let mut current_playlist = self.playlist.write().await;
    *current_playlist = p;
  }

  #[instrument(skip(self))]
  pub(crate) async fn get_queue(&self) -> impl std::ops::Deref<Target = Playlist> + '_ {
    self.queue.read().await
  }
  #[instrument(skip(self))]
  pub(crate) async fn get_mut_queue(&self) -> impl std::ops::DerefMut<Target = Playlist> + '_ {
    self.queue.write().await
  }
  #[instrument(skip(self))]
  pub(crate) async fn set_queue(&self, q: Playlist) {
    let mut queue = self.queue.write().await;
    *queue = q;
  }

  #[instrument(skip(self))]
  pub(crate) async fn get_db(&self) -> impl std::ops::Deref<Target = Rhythmdb> + '_ {
    self.db.read().await
  }
  #[instrument(skip(self))]
  pub(crate) async fn get_mut_db(&self) -> impl std::ops::DerefMut<Target = Rhythmdb> + '_ {
    self.db.write().await
  }
  #[instrument(skip(self, db))]
  pub(crate) async fn set_db(&self, db: Rhythmdb) {
    let mut pdb = self.db.write().await;
    *pdb = db;
  }

  #[instrument(skip(self))]
  pub(crate) async fn find_track_index(&self, entry: &Entry) -> Option<usize> {
    let entries = self.playlist.read().await;
    for (i, e) in entries.iter().enumerate() {
      match (entry, e.as_ref()) {
        (Entry::Song(e1), Entry::Song(e2)) => {
          if e1._internal_id == e2._internal_id {
            return Some(i);
          }
        }
        (Entry::PodcastPost(p1), Entry::PodcastPost(p2)) => {
          if p1._internal_id == p2._internal_id {
            return Some(i);
          }
        }
        _ => return None,
      }
    }
    None
  }

  #[instrument(skip(self))]
  pub(crate) async fn get_shuffle_mode(&self) -> Shuffle {
    let shuffle_mode = self.shuffle_mode.read().await;
    *shuffle_mode
  }

  #[instrument(skip(self))]
  pub(crate) async fn set_shuffle_mode(&self, mode: Shuffle) {
    let mut shuffle_mode = self.shuffle_mode.write().await;
    *shuffle_mode = mode;
  }

  #[instrument(skip(self))]
  pub(crate) async fn get_repeat_mode(&self) -> Repeat {
    let repeat_mode = self.repeat_mode.read().await;
    *repeat_mode
  }

  #[instrument(skip(self))]
  pub(crate) async fn set_repeat_mode(&self, mode: Repeat) {
    let mut repeat_mode = self.repeat_mode.write().await;
    *repeat_mode = mode;
  }

  #[instrument(skip(self))]
  pub(crate) async fn set_sender(&self, senderx: Sender<UiNotification>) {
    let mut sender = self.sender.write().await;
    *sender = Some(senderx);
  }

  #[instrument(skip(self, msg))]
  pub(crate) async fn notify_ui(&self, msg: UiNotification) -> Result<()> {
    if let Some(sender) = self.sender.read().await.clone() {
      sender.send(msg).await.into_diagnostic()?;
    }
    Ok(())
  }

  #[instrument(skip(self))]
  pub(crate) fn properties_changed(&self, properties: Vec<Property>) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().into_diagnostic()?;
    rt.spawn(async {
      let mpris_server = get_mpris_server().await.expect("mpris not found!");
      let _ = mpris_server.properties_changed(properties).await;
    });

    sleep(Duration::from_millis(50));
    rt.shutdown_background();
    Ok(())
  }

  #[instrument(skip(track_list))]
  pub(crate) fn choose_track(track_list: &[Arc<Entry>]) -> Result<(Arc<Entry>, usize)> {
    use rand::Rng;
    let index = rand::thread_rng().gen_range(0..track_list.len());
    if let Some(song) = track_list.get(index) {
      Ok((song.clone(), index))
    } else {
      miette::bail!("")
    }
  }

  #[instrument(skip(self, track_list))]
  pub(crate) async fn choose_track_last_played(
    &self,
    track_list: &[Arc<Entry>],
  ) -> Result<(Arc<Entry>, usize)> {
    use rand::seq::SliceRandom;

    let mut db = self.db.write().await;
    let first_played = db.first_played();
    let song = {
      let now: u64 = chrono::Local::now().timestamp() as u64;
      let mut rng = rand::thread_rng();
      track_list.choose_weighted(&mut rng, |track| match track.as_ref() {
        Entry::Song(song) => match song.last_played {
          Some(date) => now - date,
          None => now - first_played,
        },
        Entry::PodcastPost(song) => match song.last_played {
          Some(date) => now - date,
          None => now - first_played,
        },
        _ => 1,
      })
    }
    .into_diagnostic()?;

    let index = self.find_track_index(song).await.unwrap_or_default();
    Ok((song.clone(), index))
  }
}

impl PlayerState {
  #[instrument(skip(self))]
  pub(crate) async fn stop_track(&self) -> Result<()> {
    if let Some(pipeline) = self.get_pipeline().await {
      stop(&pipeline)?;
      self
        .notify_ui(UiNotification::Position(Duration::ZERO))
        .await?;
    }
    Ok(())
  }

  #[instrument(skip(self))]
  pub(crate) async fn play_track(&self, track: SharedEntry) -> Result<()> {
    let pipeline = start_playing(&track.get_location())?;
    self.set_pipeline(pipeline).await;
    self.set_track(track.clone()).await;
    self.properties_changed(vec![Property::Metadata((&*track).into())])?;
    self
      .notify_ui(UiNotification::Position(Duration::ZERO))
      .await?;
    Ok(())
  }

  #[instrument(skip(self))]
  pub(crate) async fn next_track(&self) -> Result<usize> {
    let mut queue = self.get_mut_queue().await;
    if !queue.queue().is_empty() {
      let get_track = self.get_track().await;
      if let Some(current_track) = get_track.as_ref() {
        queue.remove(current_track.get_location());
        self.notify_ui(UiNotification::RebuildTable).await?;
      }
    }

    let track_list = if queue.queue().is_empty() {
      self.get_playlist().await.to_vec()
    } else {
      let queue_entries = self.get_db().await.to_entries(&queue);
      if queue_entries.is_empty() {
        self.get_playlist().await.to_vec()
      } else {
        queue_entries
      }
    };

    let shuffle_mode = self.get_shuffle_mode().await;
    let repeat_mode = self.get_repeat_mode().await;
    loop {
      // Loop until play a track without errors
      let (track, index) = match (shuffle_mode, repeat_mode, queue.queue().is_empty()) {
        (_, Repeat::AllTracks, false) => (track_list[0].clone(), 0),
        (Shuffle::Next, Repeat::AllTracks, true) => {
          let get_track = self.get_track().await;
          if let Some(get_track) = get_track.as_ref() {
            let index =
              (self.find_track_index(get_track).await.unwrap_or_default() + 1) % track_list.len();
            (track_list[index].clone(), index)
          } else {
            (Arc::new(Entry::Song(SongEntry::default())), 0)
          }
        }
        (_, Repeat::CurrentTrack, _) => {
          let get_track = self.get_track().await;
          if let Some(track) = get_track.as_ref() {
            let index = self.find_track_index(track).await.unwrap_or_default();
            (track.clone(), index)
          } else {
            (Arc::new(Entry::Song(SongEntry::default())), 0)
          }
        }
        (Shuffle::Shuffle, Repeat::AllTracks, true) => PlayerState::choose_track(&track_list)?,
        (Shuffle::ShuffleLastPlayed, Repeat::AllTracks, true) => {
          self.choose_track_last_played(&track_list).await?
        }
      };

      self.stop_track().await?;
      if let Err(e) = self.play_track(track.clone()).await {
        tracing::error!("Error starting '{}': {}", &track.get_location(), e);
      // Error: continue looping.
      } else {
        // Track is currently played. We can exit this function.
        self
          .notify_ui(UiNotification::UpdateIndex(Some(index)))
          .await?;
        return Ok(index);
      }
    }
  }

  #[instrument(skip(self))]
  pub(crate) async fn track_position(&self) -> Result<u64> {
    use gstreamer::prelude::ElementExtManual;
    Ok(if let Some(pipeline) = self.get_pipeline().await {
      pipeline
        .query_position::<gstreamer::ClockTime>()
        .unwrap_or_default()
        .mseconds()
    } else {
      0
    })
  }

  #[instrument(skip(self))]
  pub(crate) async fn track_seek(&self, new_position: u64) -> Result<()> {
    use gstreamer::{prelude::ElementExtManual, SeekFlags};
    if let Some(pipeline) = self.get_pipeline().await {
      pipeline
        .seek_simple(
          SeekFlags::KEY_UNIT | SeekFlags::FLUSH,
          new_position * gstreamer::ClockTime::SECOND,
        )
        .into_diagnostic()?;
    }
    Ok(())
  }
  #[instrument(skip(self, db))]
  pub(crate) async fn update_rating(
    &self,
    db: &mut Rhythmdb,
    i: Option<usize>,
    rating: u64,
    settings: &crate::settings::Settings,
  ) -> Result<()> {
    let playlist_view = self.get_playlist().await;
    let track = &playlist_view[i.unwrap()];

    let updated_track = match track.as_ref() {
      Entry::Song(song) => {
        let mut song_copy = song.to_owned();
        song_copy.rating = Some(rating);
        Arc::new(Entry::Song(song_copy))
      }
      Entry::PodcastPost(podcast) => {
        let mut podcast_copy = podcast.to_owned();
        podcast_copy.rating = Some(rating);
        Arc::new(Entry::PodcastPost(podcast_copy))
      }
      _ => unimplemented!(),
    };
    db.update_entry(updated_track.clone());
    // to avoid the lock 3 lines below (set_track)
    let get_track = { self.get_track().await.clone() };
    if let Some(played_track) = &get_track {
      if updated_track.get_id() == played_track.get_id() {
        self.set_track(updated_track).await;
      }
    }
    db.save(settings)?;
    Ok(())
  }
}

impl From<&Entry> for Metadata {
  fn from(value: &Entry) -> Self {
    match value {
      Entry::Song(song) => Metadata::builder()
        .title(song.title.clone())
        .artist([song.artist.clone()])
        .album(song.album.clone())
        .length(Time::from_secs(song.duration.unwrap_or_default() as i64))
        .build(),
      Entry::Iradio(_) => todo!(),
      Entry::Ignore(_) => todo!(),
      Entry::PodcastFeed(_) => todo!(),
      Entry::PodcastPost(podcast) => Metadata::builder()
        .title(podcast.title.clone())
        .artist([podcast.artist.clone()])
        .album(podcast.album.clone())
        .length(Time::from_secs(podcast.duration.unwrap_or_default() as i64))
        .build(),
    }
  }
}
