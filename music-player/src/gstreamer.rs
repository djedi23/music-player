use gstreamer::{parse::launch, prelude::ElementExt, Element, State, StateChangeSuccess};
use miette::{IntoDiagnostic, Result, WrapErr};
use tracing::instrument;
use url::Url;

#[instrument]
pub(crate) fn gstreamer_init() -> Result<()> {
  // Initialize GStreamer
  gstreamer::init().into_diagnostic()
}

#[instrument]
pub(crate) fn start_playing(url: &Url) -> Result<Element> {
  let pipeline = launch(&format!("playbin3 uri={url}")).into_diagnostic()?;

  play(&pipeline).with_context(|| format!("Can play {url}"))?;
  Ok(pipeline)
}

#[instrument]
pub(crate) fn stop(pipeline: &Element) -> Result<StateChangeSuccess> {
  // Shutdown pipeline
  pipeline
    .set_state(State::Null)
    .into_diagnostic()
    .context("Unable to set the pipeline to the `Null` state")
}

#[instrument]
pub(crate) fn pause(pipeline: &Element) -> Result<StateChangeSuccess> {
  pipeline
    .set_state(State::Paused)
    .into_diagnostic()
    .context("Unable to set the pipeline to the `Pause` state")
}

#[instrument]
pub(crate) fn play(pipeline: &Element) -> Result<StateChangeSuccess> {
  pipeline
    .set_state(State::Playing)
    .into_diagnostic()
    .context("Unable to set the pipeline to the `Playing` state")
}
