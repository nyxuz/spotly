use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct Word {
    pub text: String,
    pub start: Duration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LyricsSource {
    LyricsFileWord,
    LyricsFileLine,
    SyncedLrc,
    PlainApprox,
}

impl LyricsSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LyricsFileWord => "lyricsfile-word",
            Self::LyricsFileLine => "lyricsfile-line",
            Self::SyncedLrc => "synced-lrc",
            Self::PlainApprox => "plain-approx",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Lyrics {
    pub words: Vec<Word>,
    pub source: LyricsSource,
}

impl Lyrics {
    pub fn word_at_with_timing(&self, position: Duration) -> Option<(&str, Duration)> {
        if self.words.is_empty() {
            return None;
        }

        let index = self.words.partition_point(|word| word.start <= position);

        if index == 0 {
            None
        } else {
            let word = &self.words[index - 1];

            Some((word.text.as_str(), word.start))
        }
    }
}

#[derive(Debug, Deserialize)]
struct LrcLibResponse {
    #[serde(rename = "plainLyrics")]
    plain_lyrics: Option<String>,

    #[serde(rename = "syncedLyrics")]
    synced_lyrics: Option<String>,

    #[serde(rename = "lyricsfile", alias = "lyricsFile")]
    lyrics_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LyricsFile {
    #[allow(dead_code)]
    version: Option<String>,

    #[allow(dead_code)]
    metadata: Option<LyricsFileMetadata>,

    lines: Option<Vec<LyricsFileLine>>,

    plain: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LyricsFileMetadata {
    #[allow(dead_code)]
    title: Option<String>,

    #[allow(dead_code)]
    artist: Option<String>,

    #[allow(dead_code)]
    album: Option<String>,

    #[allow(dead_code)]
    duration_ms: Option<u64>,

    #[allow(dead_code)]
    instrumental: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct LyricsFileLine {
    text: Option<String>,
    start_ms: Option<u64>,
    end_ms: Option<u64>,
    words: Option<Vec<LyricsFileWord>>,
}

#[derive(Debug, Deserialize)]
struct LyricsFileWord {
    text: Option<String>,
    start_ms: Option<u64>,

    #[allow(dead_code)]
    end_ms: Option<u64>,
}

pub fn fetch(track: &Track) -> Result<Option<Lyrics>, Box<dyn std::error::Error>> {
    if track.title.trim().is_empty() || track.artist.trim().is_empty() {
        return Ok(None);
    }

    let mut request = ureq::get("https://lrclib.net/api/get")
        .query("track_name", &track.title)
        .query("artist_name", &track.artist);

    if let Some(album) = &track.album {
        if !album.trim().is_empty() {
            request = request.query("album_name", album);
        }
    }

    if let Some(duration) = track.duration {
        request = request.query("duration", &duration.as_secs_f64().to_string());
    }

    let response = match request.call() {
        Ok(response) => response,

        Err(_) => {
            return Ok(None);
        }
    };

    let body = match response.into_body().read_to_string() {
        Ok(body) => body,

        Err(_) => {
            return Ok(None);
        }
    };

    let data: LrcLibResponse = match serde_json::from_str(&body) {
        Ok(data) => data,

        Err(_) => {
            return Ok(None);
        }
    };

    /*
     * Priority:
     *
     * 1. lyricsfile with per-word timestamps
     * 2. lyricsfile with line timestamps
     * 3. lyricsfile plain fallback
     * 4. enhanced LRC
     * 5. normal timestamped LRC
     * 6. plainLyrics fallback
     */

    if let Some(text) = data.lyrics_file.as_deref() {
        if let Some(lyrics) = parse_lyricsfile_yaml(text, track.duration) {
            return Ok(Some(lyrics));
        }
    }

    if let Some(text) = data.synced_lyrics.as_deref() {
        if let Some(lyrics) = parse_enhanced_lrc(text) {
            return Ok(Some(lyrics));
        }

        if let Some(lyrics) = parse_line_lrc_adaptive(text) {
            return Ok(Some(lyrics));
        }
    }

    if let Some(text) = data.plain_lyrics.as_deref() {
        if let Some(lyrics) = parse_plain_lyrics(text, track.duration) {
            return Ok(Some(lyrics));
        }
    }

    Ok(None)
}

fn parse_lyricsfile_yaml(text: &str, duration: Option<Duration>) -> Option<Lyrics> {
    let file: LyricsFile = match serde_yaml::from_str(text) {
        Ok(file) => file,

        Err(_) => {
            return None;
        }
    };

    let lines = file.lines.unwrap_or_default();

    let mut word_timed = Vec::new();
    let mut line_timed = Vec::new();

    for line in &lines {
        if let Some(words) = &line.words {
            let mut found_word = false;

            for word in words {
                let text = match word.text.as_deref() {
                    Some(text) => text.trim(),
                    None => continue,
                };

                if text.is_empty() {
                    continue;
                }

                let start_ms = match word.start_ms {
                    Some(value) => value,
                    None => continue,
                };

                word_timed.push(Word {
                    text: text.to_string(),
                    start: Duration::from_millis(start_ms),
                });

                found_word = true;
            }

            if found_word {
                continue;
            }
        }

        let text = match line.text.as_deref() {
            Some(text) => text.trim(),
            None => continue,
        };

        if text.is_empty() {
            continue;
        }

        let start_ms = match line.start_ms {
            Some(value) => value,
            None => continue,
        };

        line_timed.push(TimedLine {
            start: Duration::from_millis(start_ms),
            end: line.end_ms.map(Duration::from_millis),
            text: text.to_string(),
        });
    }

    /*
     * Best case:
     * lyricsfile contains exact per-word timing.
     */
    if !word_timed.is_empty() {
        word_timed.sort_by_key(|word| word.start);

        return Some(Lyrics {
            words: word_timed,
            source: LyricsSource::LyricsFileWord,
        });
    }

    /*
     * Second best:
     * lyricsfile contains line timing.
     *
     * We distribute each line's duration between
     * its words using word-length weighting.
     */
    if !line_timed.is_empty() {
        line_timed.sort_by_key(|line| line.start);

        let words = estimate_adaptive_timing(&line_timed);

        if !words.is_empty() {
            return Some(Lyrics {
                words,
                source: LyricsSource::LyricsFileLine,
            });
        }
    }

    /*
     * Some LRCLIB lyricsfile entries have:
     *
     * lines: []
     * plain: |-
     *   ...
     *
     * Use the actual Spotify duration when available.
     */
    if let Some(plain) = file.plain.as_deref() {
        if let Some(lyrics) = parse_plain_lyrics_internal(plain, duration) {
            return Some(Lyrics {
                words: lyrics.words,
                source: LyricsSource::PlainApprox,
            });
        }
    }

    None
}

#[derive(Debug, Clone)]
struct TimedLine {
    start: Duration,
    end: Option<Duration>,
    text: String,
}

fn parse_line_lrc_adaptive(text: &str) -> Option<Lyrics> {
    let mut lines = Vec::new();

    for raw_line in text.lines() {
        let line = raw_line.trim();

        if line.is_empty() {
            continue;
        }

        let mut position = 0usize;
        let mut timestamps = Vec::new();

        while position < line.len() {
            let remaining = &line[position..];

            if !remaining.starts_with('[') {
                break;
            }

            let close = match remaining.find(']') {
                Some(close) => close,
                None => break,
            };

            let timestamp_text = &remaining[1..close];

            let timestamp = match parse_timestamp(timestamp_text) {
                Some(timestamp) => timestamp,
                None => break,
            };

            timestamps.push(timestamp);

            position += close + 1;
        }

        if timestamps.is_empty() {
            continue;
        }

        let content = line[position..].trim();

        if content.is_empty() {
            continue;
        }

        for timestamp in timestamps {
            lines.push(TimedLine {
                start: timestamp,
                end: None,
                text: content.to_string(),
            });
        }
    }

    if lines.is_empty() {
        return None;
    }

    lines.sort_by_key(|line| line.start);

    lines.dedup_by(|a, b| a.start == b.start && a.text == b.text);

    let words = estimate_adaptive_timing(&lines);

    if words.is_empty() {
        None
    } else {
        Some(Lyrics {
            words,
            source: LyricsSource::SyncedLrc,
        })
    }
}

fn estimate_adaptive_timing(lines: &[TimedLine]) -> Vec<Word> {
    let mut words = Vec::new();

    for index in 0..lines.len() {
        let line = &lines[index];

        let next_start = lines.get(index + 1).map(|next| next.start);

        let line_words = split_words(&line.text);

        if line_words.is_empty() {
            continue;
        }

        let available_duration = match line.end {
            Some(end) if end > line.start => Some(end - line.start),

            _ => next_start.and_then(|next| next.checked_sub(line.start)),
        };

        let allocations = allocate_word_timing(&line_words, available_duration);

        let mut elapsed = Duration::ZERO;

        for (word, offset) in line_words.iter().zip(allocations) {
            let start = line.start.saturating_add(elapsed);

            words.push(Word {
                text: word.text.clone(),
                start,
            });

            elapsed = elapsed.saturating_add(offset);
        }
    }

    words.sort_by_key(|word| word.start);

    words
}

fn parse_plain_lyrics(text: &str, duration: Option<Duration>) -> Option<Lyrics> {
    let duration = duration?;

    if duration.is_zero() {
        return None;
    }

    parse_plain_lyrics_internal(text, Some(duration))
}

fn parse_plain_lyrics_internal(text: &str, duration: Option<Duration>) -> Option<Lyrics> {
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        return None;
    }

    let duration = match duration {
        Some(duration) => {
            if duration.is_zero() {
                return None;
            }

            duration
        }

        None => {
            /*
             * No real duration available.
             *
             * This is only a last-resort approximation.
             */
            let timed_lines: Vec<TimedLine> = lines
                .iter()
                .enumerate()
                .map(|(index, line)| TimedLine {
                    start: Duration::from_millis(index as u64 * 5_000),
                    end: None,
                    text: (*line).to_string(),
                })
                .collect();

            let words = estimate_adaptive_timing(&timed_lines);

            if words.is_empty() {
                return None;
            }

            return Some(Lyrics {
                words,
                source: LyricsSource::PlainApprox,
            });
        }
    };

    let line_count = lines.len();

    let timed_lines: Vec<TimedLine> = lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let start = duration.mul_f64(index as f64 / line_count as f64);

            let end = if index + 1 < line_count {
                Some(duration.mul_f64((index + 1) as f64 / line_count as f64))
            } else {
                Some(duration)
            };

            TimedLine {
                start,
                end,
                text: (*line).to_string(),
            }
        })
        .collect();

    let words = estimate_adaptive_timing(&timed_lines);

    if words.is_empty() {
        None
    } else {
        Some(Lyrics {
            words,
            source: LyricsSource::PlainApprox,
        })
    }
}

fn parse_enhanced_lrc(text: &str) -> Option<Lyrics> {
    let mut words = Vec::new();

    for line in text.lines() {
        let mut search_start = 0usize;

        while let Some(relative_open) = line[search_start..].find('<') {
            let open = search_start + relative_open;

            let after_open = &line[open + 1..];

            let close = match after_open.find('>') {
                Some(value) => value,
                None => break,
            };

            let timestamp_text = &after_open[..close];

            let timestamp = match parse_timestamp(timestamp_text) {
                Some(value) => value,

                None => {
                    search_start = open + 1;
                    continue;
                }
            };

            let after_timestamp = &after_open[close + 1..];

            let word_end = after_timestamp.find('<').unwrap_or(after_timestamp.len());

            let word = after_timestamp[..word_end].trim();

            if !word.is_empty() {
                let split = word.split_whitespace().collect::<Vec<_>>();

                if split.len() == 1 {
                    words.push(Word {
                        text: split[0].to_string(),
                        start: timestamp,
                    });
                } else {
                    let weights: Vec<f64> = split.iter().map(|word| word_weight(word)).collect();

                    let total_weight: f64 = weights.iter().sum();

                    let mut offset = Duration::ZERO;

                    for (word, weight) in split.iter().zip(weights) {
                        words.push(Word {
                            text: (*word).to_string(),
                            start: timestamp.saturating_add(offset),
                        });

                        let fraction = if total_weight > 0.0 {
                            weight / total_weight
                        } else {
                            1.0 / split.len() as f64
                        };

                        let step = Duration::from_secs_f64(0.08 * fraction);

                        offset = offset.saturating_add(step);
                    }
                }
            }

            search_start = open + close + 2;
        }
    }

    if words.is_empty() {
        None
    } else {
        words.sort_by_key(|word| word.start);

        Some(Lyrics {
            words,
            source: LyricsSource::SyncedLrc,
        })
    }
}

#[derive(Debug, Clone)]
struct LyricWord {
    text: String,
    weight: f64,
}

fn split_words(text: &str) -> Vec<LyricWord> {
    text.split_whitespace()
        .filter(|word| !word.is_empty())
        .map(|word| LyricWord {
            text: word.to_string(),
            weight: word_weight(word),
        })
        .collect()
}

fn word_weight(word: &str) -> f64 {
    let chars = word.chars().filter(|c| c.is_alphanumeric()).count();

    let base = match chars {
        0 => 0.5,
        1 => 0.65,
        2 => 0.80,
        3 => 0.95,
        4 => 1.05,
        5 => 1.15,
        6 => 1.25,
        7 => 1.35,
        8 => 1.45,
        9 => 1.55,
        10 => 1.65,

        _ => 1.65 + ((chars - 10) as f64 * 0.08),
    };

    let punctuation = if word.ends_with('.') || word.ends_with('!') || word.ends_with('?') {
        0.35
    } else if word.ends_with(',') || word.ends_with(';') || word.ends_with(':') {
        0.18
    } else {
        0.0
    };

    base + punctuation
}

fn allocate_word_timing(
    words: &[LyricWord],
    available_duration: Option<Duration>,
) -> Vec<Duration> {
    if words.is_empty() {
        return Vec::new();
    }

    let duration = match available_duration {
        Some(duration) if duration > Duration::ZERO => duration,

        _ => {
            return fallback_word_durations(words);
        }
    };

    let total_weight: f64 = words.iter().map(|word| word.weight).sum();

    if total_weight <= 0.0 {
        return fallback_word_durations(words);
    }

    let reserve = if duration >= Duration::from_millis(500) {
        Duration::from_millis(40)
    } else {
        Duration::ZERO
    };

    let usable = duration.saturating_sub(reserve);

    let usable_seconds = usable.as_secs_f64();

    let mut result = Vec::with_capacity(words.len());

    for word in words {
        let fraction = word.weight / total_weight;

        let seconds = usable_seconds * fraction;

        result.push(Duration::from_secs_f64(seconds.max(0.001)));
    }

    let current_total = result
        .iter()
        .copied()
        .fold(Duration::ZERO, |a, b| a.saturating_add(b));

    if let Some(last) = result.last_mut() {
        if current_total < usable {
            *last = last.saturating_add(usable - current_total);
        } else if current_total > usable {
            *last = last.saturating_sub(current_total - usable);
        }
    }

    result
}

fn fallback_word_durations(words: &[LyricWord]) -> Vec<Duration> {
    words
        .iter()
        .map(|word| {
            let milliseconds = 220.0 * word.weight;

            Duration::from_millis(milliseconds.round().clamp(120.0, 700.0) as u64)
        })
        .collect()
}

fn parse_timestamp(text: &str) -> Option<Duration> {
    let text = text.trim();

    if text.is_empty() {
        return None;
    }

    if let Some((minutes, seconds)) = text.split_once(':') {
        let minutes: u64 = minutes.parse().ok()?;

        let seconds: f64 = seconds.parse().ok()?;

        if !seconds.is_finite() || seconds < 0.0 {
            return None;
        }

        let total = minutes as f64 * 60.0 + seconds;

        return Some(Duration::from_secs_f64(total));
    }

    let seconds: f64 = text.parse().ok()?;

    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }

    Some(Duration::from_secs_f64(seconds))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_timestamp() {
        let value = parse_timestamp("01:23.456").unwrap();

        assert_eq!(value, Duration::from_millis(83_456));
    }

    #[test]
    fn parses_seconds_timestamp() {
        let value = parse_timestamp("12.500").unwrap();

        assert_eq!(value, Duration::from_millis(12_500));
    }

    #[test]
    fn parses_enhanced_lrc() {
        let lyrics = parse_enhanced_lrc(
            "[00:01.000]<00:01.000>Hello \
                 <00:01.500>world",
        )
        .unwrap();

        assert_eq!(lyrics.words.len(), 2);

        assert_eq!(lyrics.words[0].text, "Hello");

        assert_eq!(lyrics.words[1].text, "world");

        assert_eq!(lyrics.source, LyricsSource::SyncedLrc);
    }

    #[test]
    fn parses_line_lrc() {
        let lyrics = parse_line_lrc_adaptive(
            "[00:10.000]hello world\n\
                 [00:12.000]this is another line",
        )
        .unwrap();

        assert_eq!(lyrics.words.len(), 6);

        assert_eq!(lyrics.words[0].text, "hello");

        assert_eq!(lyrics.words[2].text, "this");
    }

    #[test]
    fn parses_multiple_lrc_timestamps() {
        let lyrics = parse_line_lrc_adaptive("[00:10.000][00:20.000]same text").unwrap();

        assert_eq!(lyrics.words.len(), 4);

        assert_eq!(lyrics.words[0].text, "same");

        assert_eq!(lyrics.words[2].text, "same");

        assert_eq!(lyrics.words[0].start, Duration::from_secs(10));

        assert_eq!(lyrics.words[2].start, Duration::from_secs(20));
    }

    #[test]
    fn parses_lyricsfile_yaml() {
        let yaml = r#"
version: '1.0'
lines:
  - text: hello world
    start_ms: 10000
    end_ms: 12000
    words:
      - text: hello
        start_ms: 10000
        end_ms: 11000
      - text: world
        start_ms: 11000
        end_ms: 12000
"#;

        let lyrics = parse_lyricsfile_yaml(yaml, Some(Duration::from_secs(20))).unwrap();

        assert_eq!(lyrics.words.len(), 2);

        assert_eq!(lyrics.words[0].text, "hello");

        assert_eq!(lyrics.words[1].text, "world");

        assert_eq!(lyrics.words[0].start, Duration::from_secs(10));

        assert_eq!(lyrics.words[1].start, Duration::from_millis(11_000));

        assert_eq!(lyrics.source, LyricsSource::LyricsFileWord);
    }

    #[test]
    fn lyricsfile_falls_back_to_line_timing() {
        let yaml = r#"
version: '1.0'
lines:
  - text: hello world
    start_ms: 10000
    end_ms: 12000
  - text: next line
    start_ms: 12000
    end_ms: 15000
"#;

        let lyrics = parse_lyricsfile_yaml(yaml, Some(Duration::from_secs(20))).unwrap();

        assert_eq!(lyrics.words.len(), 4);

        assert_eq!(lyrics.words[0].text, "hello");

        assert_eq!(lyrics.words[2].text, "next");

        assert_eq!(lyrics.source, LyricsSource::LyricsFileLine);
    }

    #[test]
    fn lyricsfile_plain_fallback() {
        let yaml = r#"
version: '1.0'
metadata:
  title: Test
  artist: Artist
  duration_ms: 20000
lines: []
plain: |-
  hello world
  this is a test
"#;

        let lyrics = parse_lyricsfile_yaml(yaml, Some(Duration::from_secs(20))).unwrap();

        assert_eq!(lyrics.words.len(), 6);

        assert_eq!(lyrics.source, LyricsSource::PlainApprox);

        assert_eq!(lyrics.words[0].start, Duration::ZERO);

        assert!(lyrics.words.last().unwrap().start < Duration::from_secs(20));
    }

    #[test]
    fn parses_plain_lyrics() {
        let lyrics = parse_plain_lyrics(
            "hello world\n\
                 this is a test",
            Some(Duration::from_secs(20)),
        )
        .unwrap();

        assert_eq!(lyrics.words.len(), 6);

        assert_eq!(lyrics.source, LyricsSource::PlainApprox);

        assert!(lyrics.words[0].start < lyrics.words[3].start);
    }

    #[test]
    fn slow_line_gets_more_time() {
        let lyrics = parse_line_lrc_adaptive(
            "[00:10.000]I really don't know\n\
                 [00:20.000]next line",
        )
        .unwrap();

        assert_eq!(lyrics.words.len(), 6);

        assert!(lyrics.words[3].start > Duration::from_secs(15));

        assert!(lyrics.words[4].start > lyrics.words[3].start);
    }

    #[test]
    fn word_lookup_works() {
        let lyrics = Lyrics {
            words: vec![
                Word {
                    text: "one".into(),
                    start: Duration::from_secs(1),
                },
                Word {
                    text: "two".into(),
                    start: Duration::from_secs(2),
                },
                Word {
                    text: "three".into(),
                    start: Duration::from_secs(3),
                },
            ],
            source: LyricsSource::SyncedLrc,
        };

        assert_eq!(lyrics.word_at_with_timing(Duration::from_millis(500)), None);

        assert_eq!(
            lyrics
                .word_at_with_timing(Duration::from_millis(1500))
                .map(|(word, _)| word),
            Some("one")
        );

        assert_eq!(
            lyrics
                .word_at_with_timing(Duration::from_millis(2500))
                .map(|(word, _)| word),
            Some("two")
        );
    }
}
