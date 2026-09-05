mod clock;
mod lyrics;
mod terminal;

use std::env;
use std::thread;
use std::time::{Duration, Instant};

use mpris::{Metadata, PlaybackStatus, PlayerFinder};

use clock::PlaybackClock;
use lyrics::{Lyrics, Track, fetch};

const LYRICS_LEAD: Duration = Duration::from_millis(80);

const POSITION_SYNC_INTERVAL: Duration = Duration::from_millis(100);

const HARD_DRIFT_THRESHOLD: Duration = Duration::from_millis(250);

const SOFT_DRIFT_THRESHOLD: Duration = Duration::from_millis(60);

fn main() {
    let args: Vec<String> = env::args().collect();

    let debug = args.iter().any(|arg| arg == "--debug");

    let debug_clock = args.iter().any(|arg| arg == "--debug-clock");

    if terminal::init().is_err() {
        return;
    }

    let finder = match PlayerFinder::new() {
        Ok(finder) => finder,

        Err(_) => clean_exit(),
    };

    let player = match finder.find_by_name("spotify") {
        Ok(player) => player,

        Err(_) => clean_exit(),
    };

    let metadata = match player.get_metadata() {
        Ok(metadata) => metadata,

        Err(_) => clean_exit(),
    };

    let mut current_track = match track_from_metadata(&metadata) {
        Some(track) => track,

        None => clean_exit(),
    };

    let mut lyrics = fetch(&current_track).ok().flatten();

    log_lyrics_source(debug, lyrics.as_ref());

    let status = match player.get_playback_status() {
        Ok(status) => status,

        Err(_) => clean_exit(),
    };

    let position = player.get_position().unwrap_or(Duration::ZERO);

    let rate = player.get_playback_rate().unwrap_or(1.0);

    let mut clock = PlaybackClock::new(position, status, rate);

    /*
     * Store both text and timestamp.
     *
     * This matters when the same word appears
     * more than once.
     */
    let mut last_word = None::<(String, Duration)>;

    let mut last_spotify_position = position;

    let mut last_position_sync = Instant::now();

    render_current_word(
        &clock,
        lyrics.as_ref(),
        &mut last_word,
        debug,
        Some(last_spotify_position),
    );

    loop {
        thread::sleep(Duration::from_millis(20));

        if terminal::should_quit() {
            clean_exit();
        }

        /*
         * Stop if Spotify disappears.
         */
        if finder.find_by_name("spotify").is_err() {
            clean_exit();
        }

        let metadata = match player.get_metadata() {
            Ok(metadata) => metadata,

            Err(_) => clean_exit(),
        };

        let new_track = match track_from_metadata(&metadata) {
            Some(track) => track,

            None => clean_exit(),
        };

        /*
         * Track changed.
         */
        if new_track != current_track {
            current_track = new_track;

            // แสดง fallback ทันทีระหว่างเปลี่ยนเพลง
            terminal::render(None);

            lyrics = fetch(&current_track).ok().flatten();
            log_lyrics_source(debug, lyrics.as_ref());
            last_word = None;

            let status = match player.get_playback_status() {
                Ok(status) => status,

                Err(_) => clean_exit(),
            };

            let position = player.get_position().unwrap_or(Duration::ZERO);

            let rate = player.get_playback_rate().unwrap_or(1.0);

            clock.sync(position, status, rate);

            last_spotify_position = position;

            last_position_sync = Instant::now();

            render_current_word(
                &clock,
                lyrics.as_ref(),
                &mut last_word,
                debug,
                Some(last_spotify_position),
            );

            continue;
        }

        /*
         * Playback status.
         */
        let status = match player.get_playback_status() {
            Ok(status) => status,

            Err(_) => clean_exit(),
        };

        if status != clock.status() {
            clock.set_status(status);

            last_word = None;

            if status == PlaybackStatus::Stopped {
                clean_exit();
            }

            render_current_word(
                &clock,
                lyrics.as_ref(),
                &mut last_word,
                debug,
                Some(last_spotify_position),
            );
        }

        /*
         * Playback rate.
         */
        let rate = player.get_playback_rate().unwrap_or(1.0);

        if (rate - clock.rate()).abs() > f64::EPSILON {
            clock.set_rate(rate);

            last_word = None;
        }

        /*
         * Periodically synchronize the local
         * playback clock against Spotify.
         */
        if last_position_sync.elapsed() >= POSITION_SYNC_INTERVAL {
            let spotify_position = match player.get_position() {
                Ok(position) => position,

                Err(_) => clean_exit(),
            };

            let local_position = clock.position();

            let drift = if spotify_position >= local_position {
                spotify_position - local_position
            } else {
                local_position - spotify_position
            };

            if drift >= HARD_DRIFT_THRESHOLD {
                /*
                 * Large drift:
                 * hard reset + redraw.
                 */
                clock.sync(spotify_position, status, rate);

                last_word = None;
            } else if drift >= SOFT_DRIFT_THRESHOLD {
                /*
                 * Small drift:
                 * correct the clock without
                 * unnecessarily clearing the word.
                 */
                clock.sync(spotify_position, status, rate);
            }

            last_spotify_position = spotify_position;

            last_position_sync = Instant::now();

            if debug_clock {
                let clock_position = clock.position();

                eprintln!(
                    "[CLOCK] spotify={:.3}s clock={:.3}s drift={:+.3}s",
                    spotify_position.as_secs_f64(),
                    clock_position.as_secs_f64(),
                    signed_duration(clock_position, spotify_position,),
                );
            }
        }

        render_current_word(
            &clock,
            lyrics.as_ref(),
            &mut last_word,
            debug,
            Some(last_spotify_position),
        );
    }
}

fn track_from_metadata(metadata: &Metadata) -> Option<Track> {
    let title = metadata.title()?.trim();

    if title.is_empty() {
        return None;
    }

    let artist = metadata
        .artists()
        .and_then(|artists| artists.first().map(|artist| artist.trim()))
        .filter(|artist| !artist.is_empty())?;

    let album = metadata
        .album_name()
        .map(str::trim)
        .filter(|album| !album.is_empty())
        .map(String::from);

    Some(Track {
        title: title.to_string(),
        artist: artist.to_string(),
        album,
        duration: metadata.length(),
    })
}

fn clean_exit() -> ! {
    terminal::cleanup();

    std::process::exit(0);
}

fn log_lyrics_source(debug: bool, lyrics: Option<&Lyrics>) {
    if !debug {
        return;
    }

    match lyrics {
        Some(lyrics) => {
            eprintln!(
                "[LYRICS] source={} words={}",
                lyrics.source.as_str(),
                lyrics.words.len()
            );
        }

        None => {
            eprintln!("[LYRICS] source=none");
        }
    }
}

fn render_current_word(
    clock: &PlaybackClock,
    lyrics: Option<&Lyrics>,
    last_word: &mut Option<(String, Duration)>,
    debug: bool,
    spotify_position: Option<Duration>,
) {
    let position = clock.position();

    let lookup_position = position.saturating_add(LYRICS_LEAD);

    let current = lyrics.and_then(|lyrics| lyrics.word_at_with_timing(lookup_position));

    let current_word = current.map(|(word, timestamp)| (word.to_string(), timestamp));

    match current_word {
        Some((word, timestamp)) => {
            /*
             * Compare both text and timestamp.
             *
             * This fixes repeated words such as:
             *
             * "one ... one"
             *
             * being treated as the same occurrence.
             */
            let changed = last_word
                .as_ref()
                .map(|(last_text, last_timestamp)| {
                    last_text != &word || *last_timestamp != timestamp
                })
                .unwrap_or(true);

            if changed {
                terminal::render(Some(&word));

                *last_word = Some((word.clone(), timestamp));

                if debug {
                    let spotify = spotify_position.unwrap_or(Duration::ZERO);

                    eprintln!(
                        "[SYNC] spotify={:.3}s clock={:.3}s lookup={:.3}s \
                         word=\"{}\" timestamp={:.3}s \
                         spotify_delta={:+.3}s clock_delta={:+.3}s",
                        spotify.as_secs_f64(),
                        position.as_secs_f64(),
                        lookup_position.as_secs_f64(),
                        word,
                        timestamp.as_secs_f64(),
                        signed_duration(timestamp, spotify,),
                        signed_duration(timestamp, position,),
                    );
                }
            }
        }

        None => {
            if last_word.is_some() {
                terminal::render(None);

                *last_word = None;
            }
        }
    }
}

fn signed_duration(a: Duration, b: Duration) -> f64 {
    if a >= b {
        (a - b).as_secs_f64()
    } else {
        -(b - a).as_secs_f64()
    }
}
