use gstreamer::{parse::launch, prelude::*, ClockTime, Element, State, StateChangeSuccess};
use miette::{IntoDiagnostic, Result, WrapErr};

fn main() -> Result<()> {
  gstreamer::init().into_diagnostic()?;

  //let url = "https://gstreamer.freedesktop.org/data/media/sintel_trailer-480p.webm";
  let url = "https://audio.audiomeans.fr/file/ViDfDesaZC/c26c7bb9-df15-45a4-a4e9-b2a5861da2ba.mp3?_=1715355911&amp;ps=218ee10e-4879-4a90-ae5a-1afc6cd0de8e";

  let pipeline = launch(&format!("playbin3 uri={url}")).into_diagnostic()?;

  play(&pipeline).with_context(|| format!("Can play {url}"))?;

  println!("play");

  if let Some(bus) = pipeline.bus() {
    loop {
      while let Some(msg) = bus.timed_pop(100 * ClockTime::MSECOND) {
        let _m = msg.view();
        //        println!("{m:#?}");
      }
    }
  }

  Ok(())
}

pub(crate) fn play(pipeline: &Element) -> Result<StateChangeSuccess> {
  pipeline
    .set_state(State::Playing)
    .into_diagnostic()
    .context("Unable to set the pipeline to the `Playing` state")
}
