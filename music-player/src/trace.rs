#[cfg(feature = "console")]
use miette::IntoDiagnostic;
use miette::Result;

pub fn init_tracing() -> Result<()> {
  use tracing_error::ErrorLayer;
  use tracing_subscriber::{prelude::*, EnvFilter};
  let registry = tracing_subscriber::registry()
    .with(EnvFilter::from_default_env())
    .with(ErrorLayer::default());

  #[cfg(feature = "console")]
  let registry = registry.with(
    tracing_subscriber::fmt::layer()
      .compact()
      .with_file(false)
      .with_line_number(true)
      .with_writer(std::fs::File::create("/tmp/music-player.log").into_diagnostic()?),
  );

  #[cfg(feature = "forest")]
  let registry = registry.with(
    tracing_forest::ForestLayer::default(), //      .with_writer(std::fs::File::create("/tmp/music-player-forest.log").into_diagnostic()?),
  );

  #[cfg(feature = "otel")]
  let registry = registry.with({
    opentelemetry::global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
      .with_service_name("skl_main")
      .install_simple()
      .unwrap();
    tracing_opentelemetry::layer()
      .with_tracer(tracer)
      .with_exception_fields(true)
  });

  #[cfg(feature = "tokio-console")]
  let registry = registry.with(console_subscriber::spawn());
  registry.init();

  Ok(())
}
