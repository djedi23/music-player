use chrono::{DateTime, Local, Utc};

/// Format a date as I like
pub trait HumanDate {
  fn format_from_now(&self) -> String;
}

impl HumanDate for DateTime<Local> {
  fn format_from_now(&self) -> String {
    let now = Local::now();
    let delta = now - self;

    if self.date_naive() >= now.date_naive() {
      self.format("Today %R").to_string()
    } else if delta.num_weeks() < 1 {
      self.format("%a %R").to_string()
    } else if delta.num_weeks() < 26 {
      self.format("%d %h %R").to_string()
    } else {
      self.format("%e %b %Y").to_string()
    }
  }
}

impl HumanDate for DateTime<Utc> {
  fn format_from_now(&self) -> String {
    let now = Local::now();
    let date = self.with_timezone(&Local);
    let delta = now - date;

    if date.date_naive() >= now.date_naive() {
      date.format("Today %R").to_string()
    } else if delta.num_weeks() < 1 {
      date.format("%a %R").to_string()
    } else if delta.num_weeks() < 26 {
      date.format("%d %h %R").to_string()
    } else {
      date.format("%e %b %Y").to_string()
    }
  }
}

#[cfg(test)]
mod tests {
  use chrono::TimeDelta;

  use super::*;

  #[test]
  fn format_3_minutes() {
    let date = Local::now() - TimeDelta::minutes(3);

    assert_eq!(date.format_from_now(), date.format("Today %R").to_string());
  }

  #[test]
  fn format_6_hours() {
    let date = Local::now() - TimeDelta::hours(6);

    assert_eq!(date.format_from_now(), date.format("Today %R").to_string());
  }

  #[test]
  fn format_yesterday() {
    let date = Local::now() - TimeDelta::days(1);

    assert_eq!(date.format_from_now(), date.format("%a %R").to_string());
  }

  #[test]
  fn format_last_week() {
    let date = Local::now() - TimeDelta::weeks(1);

    assert_eq!(date.format_from_now(), date.format("%d %h %R").to_string());
  }

  #[test]
  fn format_6_month() {
    let date = Local::now() - TimeDelta::weeks(27);

    assert_eq!(date.format_from_now(), date.format("%e %b %Y").to_string());
  }
}
