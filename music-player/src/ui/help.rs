use super::rendering::THEME;
use ratatui::{
  layout::Alignment,
  prelude::{Constraint, Layout, Rect},
  text::Text,
  widgets::{Block, Borders, Clear, Padding, Row, Table},
  Frame,
};
use tracing::instrument;

#[instrument]
pub(crate) fn render_help_panel(area: Rect, frame: &mut Frame<'_>) {
  let help_rows = [
    ("⎇-h", "Display this help"),
    ("⎋, ^-c", "Quit the player"),
    ("⎇-m", "Show local tracks"),
    ("⎇-p", "Show podcasts"),
    ("⎇-q", "Show queue"),
    ("⎇-e", "Enqueue the selected track"),
    ("⎇-s", "Order by search score"),
    ("⎇-t", "Order by title"),
    ("⎇-d", "Order by date"),
    ("⎇-r", "Order by rating"),
    ("⎇-l", "Order by last played"),
    ("⎇-0..5", "Rate the selected track"),
    ("⎇-o", "Toggle shuffle mode"),
    ("⎇-c", "Repeat current track"),
    ("⎇-g", "Select the current playing track"),
    ("↓,↑,⇟,⇞", "Select the tracks"),
    ("⏎", "Play the selected track"),
    ("⏯", "Play / Pause"),
    ("⏹", "Stop"),
    ("⏭", "Next track"),
    ("←, →", "Seek 5 seconds backward or forward"),
  ];
  let [help_area] = Layout::vertical([Constraint::Length(2 + help_rows.len() as u16)])
    .margin(5)
    .horizontal_margin(15)
    .areas(area);

  let help = Table::new(
    help_rows.map(|(key, text)| {
      Row::new(vec![
        Text::from(key)
          .alignment(Alignment::Right)
          .style(THEME.help_key),
        Text::from(text).style(THEME.default),
      ])
    }),
    [Constraint::Fill(1), Constraint::Fill(2)],
  )
  .block(
    Block::default()
      .style(THEME.border)
      .padding(Padding::horizontal(1))
      .borders(Borders::ALL)
      .title("Help"),
  );

  frame.render_widget(Clear, help_area);
  frame.render_widget(help, help_area);
}
