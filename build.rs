use vergen_gitcl::{BuildBuilder, Emitter, GitclBuilder};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let build = BuildBuilder::default().build_date(true).build()?;
    let git = GitclBuilder::default().sha(true).dirty(true).build()?;

    Emitter::default()
        .add_instructions(&build)?
        .add_instructions(&git)?
        .emit()?;

    Ok(())
}
