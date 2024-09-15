use super::{help::render_help_panel, Order, OrderDir, Panel, TabSelection};
use crate::{
  player_state::{Repeat, Shuffle},
  rhythmdb::{Entry, SharedEntry},
  ui::Ui,
};
use chrono::DateTime;
use gstreamer::Element;
use humandate::HumanDate;
use humantime::format_duration;
use miette::Result;
use ratatui::{
  prelude::{Constraint, Direction, Layout, Rect, Style},
  style::{Color, Modifier, Stylize},
  symbols,
  text::{Line, Span},
  widgets::{Block, BorderType, Borders, Cell, LineGauge, Padding, Paragraph, Table, Tabs},
  Frame,
};
use std::time::Duration;
use tracing::instrument;

// ⏴ 	⏵ 	⏶ 	⏷ 	⏸ 	⏹ 	⏺ 	⏻ 	⏼ ⏭ 	⏮ 	⏯
// 🔂 🔁 🔀

pub(crate) struct Theme {
  pub(crate) default: Style,
  pub(crate) default_dark: Style,
  pub(crate) primary: Style,
  pub(crate) secondary: Style,
  pub(crate) border: Style,
  pub(crate) _border_selected: Style,
  pub(crate) selected: Style,
  pub(crate) help_key: Style,
}

pub(crate) const THEME: Theme = Theme {
  default: Style::reset(), //.fg(Color::White),
  default_dark: Style::new().fg(Color::DarkGray),
  primary: Style::new().fg(Color::Magenta),
  secondary: Style::new().fg(Color::Rgb(192, 64, 192)),
  border: Style::new().fg(Color::Rgb(128, 0, 128)),
  _border_selected: Style::new().fg(Color::LightCyan),
  selected: Style::new().fg(Color::Magenta),
  help_key: Style::new().fg(Color::Green),
};

#[instrument(skip(app))]
pub(crate) fn render_ui(
  frame: &mut Frame<'_>,
  app: &mut Ui<'_>,
  pipeline: &Element,
  track_entry: &Entry,
  shuffle_mode: Shuffle,
  repeat_mode: Repeat,
) -> Result<()> {
  let area = frame.area();
  let [title_area, search_area, table_area, control_area] = Layout::default()
    .direction(Direction::Vertical)
    .constraints(vec![
      Constraint::Length(1),
      Constraint::Length(3),
      Constraint::Fill(1),
      Constraint::Length(4),
    ])
    .areas(area);

  let [title_area, _filler_, shuffle_area, reapeat_area, tabs_area] = Layout::default()
    .direction(Direction::Horizontal)
    .constraints(vec![
      Constraint::Length(15),
      Constraint::Fill(1),
      Constraint::Length(2),
      Constraint::Length(2),
      Constraint::Length(25),
    ])
    .areas(title_area);

  // Top bar
  let title_paragraph = Paragraph::new("Music player");
  frame.render_widget(title_paragraph, title_area);
  render_tabs(frame, tabs_area, app.selected_tab);
  render_shuffle(frame, shuffle_area, shuffle_mode);
  render_repeat(frame, reapeat_area, repeat_mode);

  // Search
  let search = Paragraph::new(Line::from(vec![
    Span::from(app.search.clone()),
    Span::from("_".to_string()).style(THEME.secondary.add_modifier(Modifier::SLOW_BLINK)),
  ]))
  .style(THEME.default)
  .block(
    Block::new()
      .borders(Borders::ALL)
      .border_type(BorderType::Rounded)
      .title("Search")
      .style(THEME.border),
  );
  frame.render_widget(search, search_area);
  frame.render_stateful_widget(&app.table, table_area, &mut app.table_state);

  // Control
  {
    let elapsed_duration = app.get_track_elapsed_duration(pipeline);
    let info = Paragraph::new(match track_entry {
      Entry::Iradio(_) => todo!(),
      Entry::Ignore(_) => todo!(),
      Entry::PodcastFeed(_) => todo!(),
      Entry::Song(song) => format!("{} - {}", song.title, song.artist,),
      Entry::PodcastPost(podcast) => format!("{} - {}", podcast.title, podcast.album,),
    })
    .block(
      Block::default()
        .padding(Padding::horizontal(1))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(THEME.border),
    )
    .style(THEME.default);
    frame.render_widget(info, control_area);

    let [_not_used_, second_line] = Layout::default()
      .direction(Direction::Vertical)
      .margin(1)
      .horizontal_margin(2)
      .constraints(vec![Constraint::Length(2), Constraint::Length(1)])
      .areas(control_area);
    let duration = track_entry.get_duration();
    let ratio = elapsed_duration.as_secs_f64() / duration as f64;
    let indicatif = LineGauge::default()
      .filled_style(THEME.primary.add_modifier(Modifier::BOLD))
      .line_set(symbols::line::THICK)
      .label(format!(
        "{} / {}",
        format_duration(elapsed_duration),
        format_duration(Duration::from_secs(duration)),
      ))
      .style(THEME.default_dark)
      .ratio(if ratio > 1.0 {
        1.0
      } else if ratio < 0.0 || ratio.is_nan() {
        0.0
      } else {
        ratio
      });
    frame.render_widget(indicatif, second_line);

    if app.panel == Panel::Help {
      render_help_panel(area, frame);
    }
    Ok(())
  }
}

#[instrument]
fn render_tabs(frame: &mut Frame<'_>, tabs_area: Rect, selected_tab: TabSelection) {
  let music = vec![
    Span::styled("M", THEME.default_dark.add_modifier(Modifier::UNDERLINED)),
    Span::raw("usic"),
  ];
  let podcasts = vec![
    Span::styled("P", THEME.default_dark.add_modifier(Modifier::UNDERLINED)),
    Span::raw("odcats"),
  ];
  let queue = vec![
    Span::styled("Q", THEME.default_dark.add_modifier(Modifier::UNDERLINED)),
    Span::raw("ueue"),
  ];

  let tabs = Tabs::new(vec![music, podcasts, queue])
    .style(THEME.default_dark)
    .highlight_style(THEME.selected)
    .select(selected_tab as usize);
  frame.render_widget(tabs, tabs_area);
}

#[instrument]
fn render_shuffle(frame: &mut Frame<'_>, area: Rect, selected: Shuffle) {
  let widget = Paragraph::new(match selected {
    Shuffle::Next => "⇶",
    Shuffle::Shuffle => "🔀",
    Shuffle::ShuffleLastPlayed => "🎜",
  })
  .style(THEME.default_dark);

  frame.render_widget(widget, area);
}

#[instrument]
fn render_repeat(frame: &mut Frame<'_>, area: Rect, selected: Repeat) {
  let widget = Paragraph::new(match selected {
    Repeat::AllTracks => "🔁",
    Repeat::CurrentTrack => "🔂",
  })
  .style(THEME.default_dark);
  frame.render_widget(widget, area);
}

#[instrument(skip(entries))]
pub(crate) fn render_table<'a>(
  entries: &[SharedEntry],
  order_by: Order,
  order_dir: OrderDir,
  current_track: &Option<SharedEntry>,
  selected_tab: TabSelection,
) -> (usize, Table<'a>, Option<usize>) {
  use ratatui::widgets::Row;

  let mut current_index = None;
  let rows: Vec<Row> = entries
    .iter()
    .enumerate()
    .map(|(index, entry)| {
      Row::new(match (entry.as_ref(), selected_tab) {
        (Entry::Iradio(_), _) => todo!(),
        (Entry::Ignore(_), _) => unimplemented!(),
        (Entry::PodcastFeed(_), _) => todo!(),
        (Entry::Song(s), _) => {
          if let Some(ct) = &current_track {
            if let Entry::Song(current_track) = ct.as_ref() {
              if s._internal_id == current_track._internal_id {
                current_index = Some(index);
              }
            }
          }
          vec![
            s.title.to_owned(),
            s.artist.to_owned(),
            s.album.to_owned(),
            format_duration(Duration::from_secs(s.duration.unwrap_or_default())).to_string(),
            rating(s.rating),
            if let Some(lp) = s.last_played {
              DateTime::from_timestamp(lp as i64, 0)
                .unwrap_or_default()
                .format_from_now()
            } else {
              "-".to_string()
            },
          ]
        }
        (Entry::PodcastPost(p), TabSelection::Podcast) => {
          if let Some(ct) = &current_track {
            if let Entry::PodcastPost(current_track) = ct.as_ref() {
              if p._internal_id == current_track._internal_id {
                current_index = Some(index);
              }
            }
          }
          vec![
            DateTime::from_timestamp(p.post_time.unwrap_or_default() as i64, 0)
              .unwrap_or_default()
              .format_from_now()
              .to_string(),
            p.title.to_owned(),
            p.album.to_owned(),
            format_duration(Duration::from_secs(p.duration.unwrap_or_default())).to_string(),
            rating(p.rating),
            if let Some(lp) = p.last_played {
              DateTime::from_timestamp(lp as i64, 0)
                .unwrap_or_default()
                .format_from_now()
                .to_string()
            } else {
              "-".to_string()
            },
          ]
        }
        (Entry::PodcastPost(p), _) => {
          if let Some(ct) = &current_track {
            if let Entry::PodcastPost(current_track) = ct.as_ref() {
              if p._internal_id == current_track._internal_id {
                current_index = Some(index);
              }
            }
          }
          vec![
            p.title.to_owned(),
            p.artist.to_owned(),
            p.album.to_owned(),
            format_duration(Duration::from_secs(p.duration.unwrap_or_default())).to_string(),
            rating(p.rating),
            if let Some(lp) = p.last_played {
              DateTime::from_timestamp(lp as i64, 0)
                .unwrap_or_default()
                .format_from_now()
                .to_string()
            } else {
              "-".to_string()
            },
          ]
        }
      })
      .style(THEME.default)
    })
    .collect();

  let widths = match selected_tab {
    TabSelection::Podcast => [
      Constraint::Length(14),
      Constraint::Fill(3),
      Constraint::Fill(1),
      Constraint::Length(6),
      Constraint::Length(6),
      Constraint::Length(14),
    ],
    _ => [
      Constraint::Fill(3),
      Constraint::Fill(2),
      Constraint::Fill(1),
      Constraint::Length(6),
      Constraint::Length(6),
      Constraint::Length(14),
    ],
  };

  let rows_len = rows.len();
  let table = Table::default()
    .rows(rows)
    .widths(widths)
    .column_spacing(1)
    .header(
      Row::new(match selected_tab {
        TabSelection::Podcast => vec![
          "Date".into(),
          Cell::from(Line::from(vec![
            Span::raw("T").add_modifier(Modifier::UNDERLINED),
            Span::raw("itle"),
            match (order_by, order_dir) {
              (Order::Title, OrderDir::Asc) => Span::raw(" ⏶"),
              (Order::Title, OrderDir::Desc) => Span::raw(" ⏷"),
              _ => Span::raw(""),
            },
          ])),
          "Feed".into(),
          "Duration".into(),
          Cell::from(Line::from(vec![
            Span::raw("R").add_modifier(Modifier::UNDERLINED),
            Span::raw("ating"),
            match (order_by, order_dir) {
              (Order::Rating, OrderDir::Asc) => Span::raw(" ⏶"),
              (Order::Rating, OrderDir::Desc) => Span::raw(" ⏷"),
              _ => Span::raw(""),
            },
          ])),
          Cell::from(Line::from(vec![
            Span::raw("L").add_modifier(Modifier::UNDERLINED),
            Span::raw("ast Played"),
            match (order_by, order_dir) {
              (Order::LastPlayed, OrderDir::Asc) => Span::raw(" ⏶"),
              (Order::LastPlayed, OrderDir::Desc) => Span::raw(" ⏷"),
              _ => Span::raw(""),
            },
          ])),
        ],

        _ => vec![
          Cell::from(Line::from(vec![
            Span::raw("T").add_modifier(Modifier::UNDERLINED),
            Span::raw("itle"),
            match (order_by, order_dir) {
              (Order::Title, OrderDir::Asc) => Span::raw(" ⏶"),
              (Order::Title, OrderDir::Desc) => Span::raw(" ⏷"),
              _ => Span::raw(""),
            },
          ])),
          "Artist".into(),
          "Album".into(),
          "Duration".into(),
          Cell::from(Line::from(vec![
            Span::raw("R").add_modifier(Modifier::UNDERLINED),
            Span::raw("ating"),
            match (order_by, order_dir) {
              (Order::Rating, OrderDir::Asc) => Span::raw(" ⏶"),
              (Order::Rating, OrderDir::Desc) => Span::raw(" ⏷"),
              _ => Span::raw(""),
            },
          ])),
          Cell::from(Line::from(vec![
            Span::raw("L").add_modifier(Modifier::UNDERLINED),
            Span::raw("ast Played"),
            match (order_by, order_dir) {
              (Order::LastPlayed, OrderDir::Asc) => Span::raw(" ⏶"),
              (Order::LastPlayed, OrderDir::Desc) => Span::raw(" ⏷"),
              _ => Span::raw(""),
            },
          ])),
        ],
      })
      .style(THEME.default_dark.bold()),
    )
    .block(
      Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .style(THEME.border)
        .title_bottom(
          Line::from(pluralizer::pluralize("track", rows_len as isize, true)).right_aligned(),
        ),
    )
    .highlight_style(THEME.selected)
    .highlight_symbol(">>");
  (rows_len, table, current_index)
}

#[instrument]
fn rating(rating: Option<u64>) -> String {
  match rating {
    Some(5) => "★★★★★",
    Some(4) => "★★★★☆",
    Some(3) => "★★★☆☆",
    Some(2) => "★★☆☆☆",
    Some(1) => "★☆☆☆☆",
    Some(_) => "☆☆☆☆☆",
    None => "☆☆☆☆☆",
  }
  .into()
}
