mod config;
mod spotify;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::widgets::Paragraph;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = config::load()?;
    let cache_dir = config::cache_dir();

    let login = spotify::auth::login(&cache_dir, &cfg.client_id).await?;
    spotify::auth::connect(&login.session, login.credentials).await?;
    println!("connected as {}", login.session.username());
    println!(
        "web api token ok, expires at {}",
        login.web_token.expires_at
    );

    let mut terminal = ratatui::init();
    let result = run(&mut terminal);
    ratatui::restore();
    login.session.shutdown();
    result?;
    Ok(())
}

fn run(terminal: &mut DefaultTerminal) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| {
            let text = Paragraph::new("lightify - press q to quit");
            frame.render_widget(text, frame.area());
        })?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press && key.code == KeyCode::Char('q') {
                return Ok(());
            }
        }
    }
}
