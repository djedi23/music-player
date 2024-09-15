use crate::{
  playlists::Playlist,
  settings::Settings,
  ui::{Order, OrderDir},
};
use fuzzy_matcher::{skim::SkimMatcherV2, FuzzyMatcher};
use id3::Tag;
use itertools::Itertools;
use miette::{IntoDiagnostic, Result};
use quick_xml::{de::from_reader, impl_deserialize_for_internally_tagged_enum};
use serde::{Deserialize, Serialize};
use std::{fs::File, io::BufReader, str::FromStr, sync::Arc};
use tracing::instrument;
use url::Url;

pub(crate) type SharedEntry = Arc<Entry>;
pub(crate) type EntryList = Vec<SharedEntry>;

#[derive(Serialize, Deserialize)]
#[serde(rename = "rhythmdb")]
pub(crate) struct Rhythmdb {
  #[serde(rename = "@version")]
  version: String,
  entry: EntryList,
  #[serde(skip)]
  first_played: u64,
}

impl Rhythmdb {
  #[instrument]
  pub fn new() -> Rhythmdb {
    Rhythmdb {
      version: String::new(),
      entry: vec![],
      first_played: 0,
    }
  }

  #[instrument(skip(self))]
  pub fn update_entry(&mut self, entry: SharedEntry) -> SharedEntry {
    let mut index = 0;
    for (i, e) in self.entry.iter().enumerate() {
      match (entry.as_ref(), e.as_ref()) {
        (Entry::Song(e1), Entry::Song(e2)) => {
          if e1._internal_id == e2._internal_id {
            index = i;
            break;
          }
        }
        (Entry::PodcastPost(p1), Entry::PodcastPost(p2)) => {
          if p1._internal_id == p2._internal_id {
            index = i;
            break;
          }
        }
        _ => {}
      }
    }
    self.entry[index] = entry.clone();
    entry
  }

  #[instrument(skip(self))]
  pub fn first_played(&mut self) -> u64 {
    if self.first_played > 0 {
      return self.first_played;
    }

    let mut min = u64::MAX;
    for entry in self.entry.iter() {
      match entry.as_ref() {
        Entry::Song(entry) => {
          if let Some(lp) = entry.last_played {
            min = min.min(lp);
          }
        }
        Entry::PodcastPost(entry) => {
          if let Some(lp) = entry.last_played {
            min = min.min(lp);
          }
        }
        _ => {}
      }
    }
    self.first_played = min;
    min
  }

  pub(crate) fn show_ignored_entries(config: &Settings) -> Result<()> {
    let db = Rhythmdb::load(config)?;
    let ignore_entries = db.filter_by_ignore();
    for entry in ignore_entries {
      println!("{}", toml::to_string_pretty(&entry).into_diagnostic()?);
    }
    Ok(())
  }

  pub(crate) fn clean_ignored_entries(config: &Settings) -> Result<()> {
    let db = Rhythmdb::load(config)?;
    let new_db = Rhythmdb {
      version: db.version,
      entry: db
        .entry
        .into_iter()
        .filter(|e| !matches!(e.as_ref(), Entry::Ignore(_)))
        .collect(),
      first_played: db.first_played,
    };
    new_db.save(config)
  }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase", tag = "@type")]
pub(crate) enum Entry {
  Iradio(IRadioEntry),
  Ignore(IIgnoreEntry),
  #[serde(rename = "podcast-feed")]
  PodcastFeed(PodcastFeedEntry),
  Song(SongEntry),
  #[serde(rename = "podcast-post")]
  PodcastPost(PodcastPostentry),
}

impl Entry {
  #[instrument(skip(self))]
  pub(crate) fn get_id(&self) -> u64 {
    match self {
      Entry::Iradio(_) => todo!(),
      Entry::Ignore(_) => todo!(),
      Entry::PodcastFeed(_) => todo!(),
      Entry::Song(song) => song._internal_id,
      Entry::PodcastPost(p) => p._internal_id,
    }
  }

  #[instrument(skip(self))]
  pub(crate) fn get_location(&self) -> Url {
    match self {
      Entry::Iradio(r) => r.location.clone(),
      Entry::Ignore(i) => i.location.clone(),
      Entry::PodcastFeed(p) => p.location.clone(),
      Entry::Song(song) => song.location.clone(),
      Entry::PodcastPost(p) => p.location.clone(),
    }
  }

  #[instrument(skip(self))]
  pub(crate) fn get_duration(&self) -> u64 {
    match self {
      Entry::Iradio(_) => todo!(),
      Entry::Ignore(_) => todo!(),
      Entry::PodcastFeed(_) => todo!(),
      Entry::Song(song) => song.duration.unwrap_or_default(),
      Entry::PodcastPost(podcast) => podcast.duration.unwrap_or_default(),
    }
  }

  #[instrument(skip(self))]
  pub(crate) fn get_hidden(&self) -> bool {
    (match self {
      Entry::Ignore(ignore) => ignore.hidden.unwrap_or_default(),
      Entry::Song(song) => song.hidden.unwrap_or_default(),
      Entry::PodcastPost(podcast) => podcast.hidden.unwrap_or_default(),
      _ => 0,
    } == 1)
  }

  #[instrument(skip(self))]
  pub(crate) fn get_date(&self) -> u64 {
    match self {
      Entry::Iradio(_) => todo!(),
      Entry::Ignore(_) => todo!(),
      Entry::PodcastFeed(_) => todo!(),
      Entry::Song(song) => song.date,
      Entry::PodcastPost(podcast) => podcast.post_time.unwrap_or_default(),
    }
  }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IRadioEntry {
  title: String,
  genre: String,
  artist: String,
  album: String,
  location: Url,
  #[serde(skip_serializing_if = "Option::is_none")]
  mtime: Option<u64>,
  #[serde(rename = "last-seen")]
  #[serde(skip_serializing_if = "Option::is_none")]
  last_seen: Option<u64>,
  date: u64,
  #[serde(rename = "media-type")]
  media_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct IIgnoreEntry {
  title: String,
  genre: String,
  artist: String,
  album: String,
  location: Url,
  #[serde(skip_serializing_if = "Option::is_none")]
  mtime: Option<u64>,
  #[serde(rename = "last-seen")]
  #[serde(skip_serializing_if = "Option::is_none")]
  last_seen: Option<u64>,
  date: u64,
  #[serde(rename = "media-type")]
  media_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  hidden: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PodcastFeedEntry {
  title: String,
  genre: String,
  artist: String,
  album: String,
  location: Url,
  #[serde(rename = "last-seen")]
  last_seen: Option<u64>,
  date: u64,
  #[serde(rename = "media-type")]
  media_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  status: Option<String>,
  description: String,
  subtitle: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  summary: Option<String>,
  lang: String,
  copyright: String,
  image: String,
  #[serde(rename = "post-time", skip_serializing_if = "Option::is_none")]
  post_time: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  comment: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SongEntry {
  #[serde(skip_serializing, default = "gen_internal_id")]
  pub(crate) _internal_id: u64,
  pub(crate) title: String,
  genre: String,
  pub(crate) artist: String,
  pub(crate) album: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[serde(rename = "track-number")]
  track_number: Option<u64>,
  #[serde(rename = "track-total")]
  #[serde(skip_serializing_if = "Option::is_none")]
  track_total: Option<u64>,
  #[serde(rename = "disc-number")]
  #[serde(skip_serializing_if = "Option::is_none")]
  disc_number: Option<u64>,
  #[serde(rename = "disc-total")]
  #[serde(skip_serializing_if = "Option::is_none")]
  disc_total: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) duration: Option<u64>,
  #[serde(rename = "file-size")]
  file_size: String,
  pub(crate) location: Url,
  #[serde(skip_serializing_if = "Option::is_none")]
  mountpoint: Option<Url>,
  mtime: u64,
  #[serde(rename = "first-seen")]
  first_seen: u64,
  #[serde(rename = "last-seen")]
  last_seen: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) rating: Option<u64>,
  #[serde(rename = "play-count")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) play_count: Option<u64>,
  #[serde(rename = "last-played")]
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) last_played: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  bitrate: Option<u64>,
  date: u64,
  #[serde(rename = "media-type")]
  media_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  hidden: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  comment: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "mb-trackid")]
  mb_trackid: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "mb-artistid")]
  mb_artistid: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "mb-albumid")]
  mb_albumid: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "mb-albumartistid")]
  mb_albumartistid: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "mb-artistsortname")]
  mb_artistsortname: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "album-sortname")]
  album_sortname: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "album-artist")]
  album_artist: Option<String>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "beats-per-minute")]
  beats_per_minute: Option<String>,
  composer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PodcastPostentry {
  #[serde(skip_serializing, default = "gen_internal_id")]
  pub(crate) _internal_id: u64,
  pub(crate) title: String,
  genre: String,
  pub(crate) artist: String,
  pub(crate) album: String,
  #[serde(rename = "track-number")]
  #[serde(skip_serializing_if = "Option::is_none")]
  track_number: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) duration: Option<u64>,
  #[serde(rename = "file-size")]
  #[serde(skip_serializing_if = "Option::is_none")]
  file_size: Option<u64>,
  pub(crate) location: Url,
  #[serde(skip_serializing_if = "Option::is_none")]
  mountpoint: Option<Url>,
  #[serde(rename = "first-seen")]
  first_seen: u64,
  #[serde(skip_serializing_if = "Option::is_none", rename = "last-seen")]
  last_seen: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub(crate) rating: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none", rename = "play-count")]
  pub(crate) play_count: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  #[serde(rename = "last-played")]
  pub(crate) last_played: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  bitrate: Option<u64>,
  pub(crate) date: u64,
  #[serde(rename = "media-type")]
  media_type: String,
  #[serde(skip_serializing_if = "Option::is_none")]
  hidden: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  status: Option<u64>,
  description: String,
  subtitle: Url,
  #[serde(skip_serializing_if = "Option::is_none")]
  summary: Option<String>,
  lang: String,
  copyright: String,
  image: String,
  #[serde(rename = "post-time", skip_serializing_if = "Option::is_none")]
  pub(crate) post_time: Option<u64>,
  #[serde(skip_serializing_if = "Option::is_none")]
  comment: Option<String>,
}

impl_deserialize_for_internally_tagged_enum! {
    Entry, "@type",
    ("iradio" => Iradio(IRadioEntry)),
    ("ignore" => Ignore(IIgnoreEntry)),
    ("podcast-feed" => PodcastFeed(PodcastFeedEntry)),
    ("song" => Song(SongEntry)),
    ("podcast-post" => PodcastPost(PodcastPostentry)),
}

impl Default for SongEntry {
  fn default() -> Self {
    Self {
      _internal_id: gen_internal_id(),
      title: Default::default(),
      genre: Default::default(),
      artist: Default::default(),
      album: Default::default(),
      track_number: Default::default(),
      track_total: Default::default(),
      duration: Default::default(),
      file_size: Default::default(),
      location: Url::from_str("file:///").expect("Default URL"),
      mtime: Default::default(),
      first_seen: Default::default(),
      last_seen: Default::default(),
      play_count: Default::default(),
      last_played: Default::default(),
      bitrate: Default::default(),
      date: Default::default(),
      media_type: Default::default(),
      comment: Default::default(),
      composer: Default::default(),
      beats_per_minute: Default::default(),
      album_artist: Default::default(),
      disc_number: Default::default(),
      disc_total: Default::default(),
      rating: Default::default(),
      mountpoint: Default::default(),
      hidden: Default::default(),
      mb_artistsortname: Default::default(),
      album_sortname: Default::default(),
      mb_trackid: Default::default(),
      mb_artistid: Default::default(),
      mb_albumid: Default::default(),
      mb_albumartistid: Default::default(),
    }
  }
}

impl From<Tag> for SongEntry {
  #[allow(clippy::field_reassign_with_default)]
  #[instrument]
  fn from(tag: Tag) -> Self {
    use id3::TagLike;
    let mut song = SongEntry::default();
    song.title = tag.title().unwrap_or_default().to_string();
    song.duration = tag.duration().map(|d| d as u64);
    song
  }
}

impl Rhythmdb {
  #[instrument]
  pub(crate) fn load(settings: &Settings) -> Result<Rhythmdb> {
    let file = File::open(&settings.playlist_path).into_diagnostic()?;
    let reader = BufReader::new(file);

    from_reader(reader).into_diagnostic()
  }

  #[instrument(skip(self))]
  pub(crate) fn save(&self, settings: &Settings) -> Result<()> {
    use memmap2::MmapMut;
    use quick_xml::se::Serializer;
    use std::fs::OpenOptions;

    let mut buffer = String::new();
    let ser = Serializer::new(&mut buffer);
    self.serialize(ser).into_diagnostic()?;

    let file = OpenOptions::new()
      .read(true)
      .write(true)
      .open(&settings.playlist_path)
      .into_diagnostic()?;
    let slice = buffer.as_bytes();
    file.set_len(slice.len() as u64).into_diagnostic()?;

    let mut mmap = unsafe { MmapMut::map_mut(&file).into_diagnostic()? };
    mmap.copy_from_slice(slice);

    Ok(())
  }

  #[instrument(skip(self))]
  pub(crate) fn find_url(&self, url: &Url) -> Option<SharedEntry> {
    for e in &self.entry {
      if &e.get_location() == url {
        if e.get_hidden() {
          return None;
        } else {
          return Some(e.clone());
        }
      }
    }
    None
  }

  #[instrument(skip(self, order_by))]
  pub(crate) fn filter_by_song(
    &self,
    search: &str,
    order_by: Order,
    order_dir: OrderDir,
  ) -> EntryList {
    tracing::trace!("[{search}]");
    let matcher = SkimMatcherV2::default().smart_case();
    let sort_fn = match (order_by, order_dir) {
      (Order::Default, OrderDir::Asc) => {
        |(a, _): &(i64, &SharedEntry), (b, _): &(i64, &SharedEntry)| Ord::cmp(&a, &b)
      }
      (Order::Default, OrderDir::Desc) => {
        |(a, _): &(i64, &SharedEntry), (b, _): &(i64, &SharedEntry)| Ord::cmp(&b, &a)
      }
      (Order::Title, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&a.title, &b.title),
          _ => unimplemented!(),
        }
      }
      (Order::Title, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&b.title, &a.title),
          _ => unimplemented!(),
        }
      }
      (Order::Date, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&a.first_seen, &b.first_seen),
          _ => unimplemented!(),
        }
      }
      (Order::Date, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&b.first_seen, &a.first_seen),
          _ => unimplemented!(),
        }
      }
      (Order::Rating, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&a.rating, &b.rating),
          _ => unimplemented!(),
        }
      }
      (Order::Rating, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&b.rating, &a.rating),
          _ => unimplemented!(),
        }
      }
      (Order::LastPlayed, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&a.last_played, &b.last_played),
          _ => unimplemented!(),
        }
      }
      (Order::LastPlayed, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::Song(a), Entry::Song(b)) => Ord::cmp(&b.last_played, &a.last_played),
          _ => unimplemented!(),
        }
      }
    };

    self
      .entry
      .iter()
      .filter_map(|entry| match entry.as_ref() {
        Entry::Song(ref song) => {
          if let Some(1) = song.hidden {
            None
          } else if search.is_empty() {
            Some((1, entry))
          } else {
            let song_match = matcher.fuzzy_match(&song.title, search);
            let artist_match = matcher.fuzzy_match(&song.artist, search);
            let album_match = matcher.fuzzy_match(&song.album, search);
            let score = 4 * song_match.unwrap_or_default()
              + 2 * artist_match.unwrap_or_default()
              + album_match.unwrap_or_default();
            if score > 00 {
              Some((score, entry))
            } else {
              None
            }
          }
        }
        _ => None,
      })
      .sorted_by(sort_fn)
      .map(|(_, entry)| entry)
      .cloned()
      .collect()
  }

  #[instrument(skip(self))]
  pub(crate) fn filter_by_ignore(&self) -> Vec<IIgnoreEntry> {
    self
      .entry
      .iter()
      .cloned()
      .filter_map(|e| match e.as_ref() {
        Entry::Ignore(s) => Some(s.clone()),
        _ => None,
      })
      .collect()
  }

  #[instrument(skip(self))]
  pub(crate) fn filter_by_podcast(
    &self,
    search: &str,
    order_by: Order,
    order_dir: OrderDir,
  ) -> EntryList {
    let matcher = SkimMatcherV2::default().smart_case();
    let sort_fn = match (order_by, order_dir) {
      (Order::Default, OrderDir::Asc) => {
        |(a, _): &(i64, &SharedEntry), (b, _): &(i64, &SharedEntry)| Ord::cmp(&a, &b)
      }
      (Order::Default, OrderDir::Desc) => {
        |(a, _): &(i64, &SharedEntry), (b, _): &(i64, &SharedEntry)| Ord::cmp(&b, &a)
      }
      (Order::Title, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&a.title, &b.title),
          _ => unimplemented!(),
        }
      }
      (Order::Title, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&b.title, &a.title),
          _ => unimplemented!(),
        }
      }
      (Order::Date, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&a.post_time, &b.post_time),
          _ => unimplemented!(),
        }
      }
      (Order::Date, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&b.post_time, &a.post_time),
          _ => unimplemented!(),
        }
      }
      (Order::Rating, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&a.rating, &b.rating),
          _ => unimplemented!(),
        }
      }
      (Order::Rating, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&b.rating, &a.rating),
          _ => unimplemented!(),
        }
      }
      (Order::LastPlayed, OrderDir::Asc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&a.last_played, &b.last_played),
          _ => unimplemented!(),
        }
      }
      (Order::LastPlayed, OrderDir::Desc) => {
        |(_, a): &(i64, &SharedEntry), (_, b): &(i64, &SharedEntry)| match (a.as_ref(), b.as_ref()) {
          (Entry::PodcastPost(a), Entry::PodcastPost(b)) => Ord::cmp(&b.last_played, &a.last_played),
          _ => unimplemented!(),
        }
      }
    };
    self
      .entry
      .iter()
      .filter_map(|entry| match entry.as_ref() {
        Entry::PodcastPost(ref podcast) => {
          if let Some(1) = podcast.hidden {
            None
          } else if search.is_empty() {
            Some((entry.get_date() as i64, entry))
          } else {
            let title_match = matcher.fuzzy_match(&podcast.title, search);
            let album_match = matcher.fuzzy_match(&podcast.album, search);
            let score = title_match.unwrap_or_default() + 3 * album_match.unwrap_or_default();
            if score > 00 {
              Some((score, entry))
            } else {
              None
            }
          }
        }
        _ => None,
      })
      .sorted_by(sort_fn)
      .map(|(_, entry)| entry)
      .cloned()
      .collect()
  }

  pub(crate) fn to_entries(&self, value: &Playlist) -> Vec<SharedEntry> {
    match value {
      Playlist::Queue(q) => q
        .location
        .iter()
        .filter_map(|url| self.find_url(url))
        .collect(),
      _ => unimplemented!(),
    }
  }
}

fn gen_internal_id() -> u64 {
  rand::random()
}
