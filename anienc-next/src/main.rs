use eyre::Result;
use std::env;

fn main() -> Result<()> {
    color_eyre::install()?;
    femme::with_level(
        env::var("RUST_LOG").as_deref().unwrap_or("info").parse()?,
    );

    Ok(())
}
