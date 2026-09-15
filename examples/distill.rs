use rust_domdistiller::{apply_for_file, Options};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut arguments = std::env::args_os().skip(1);
    let file = arguments
        .next()
        .ok_or("usage: distill FILE [ORIGINAL_URL]")?;
    let original_url = arguments
        .next()
        .map(|value| {
            value
                .into_string()
                .map_err(|_| "original URL must be valid UTF-8")
        })
        .transpose()?;
    if arguments.next().is_some() {
        return Err("usage: distill FILE [ORIGINAL_URL]".into());
    }
    let result = apply_for_file(
        file,
        &Options {
            original_url,
            ..Options::default()
        },
    )?;
    eprintln!("{} ({} words)", result.title, result.word_count);
    println!("{}", result.node.to_html());
    Ok(())
}
