mod action;
mod app;
mod config;
mod message;
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

    let api = spotify::api::SpotifyApi::new(cfg.client_id.clone(), cache_dir, login.web_token);

    let playlists = api.my_playlists().await?;
    println!("{} playlists", playlists.len());

    if let Some(first) = playlists.first() {
        let page = api.playlist_tracks(&first.id).await?;
        println!(
            "{}: {} tracks on page 1, more: {}",
            first.name,
            page.tracks.len(),
            page.next.is_some()
        );
    }

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
