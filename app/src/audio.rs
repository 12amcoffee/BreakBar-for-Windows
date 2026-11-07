//! Sound playback on its own thread (the audio output handle cannot move between threads).
//! No sound device, or a bad file, just means silence: nothing here may stop the timer.
use rodio::{Decoder, OutputStream, Sink};
use std::io::Cursor;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sound {
    Windup,
    Ding,
    Tick,
}

impl Sound {
    fn bytes(self) -> &'static [u8] {
        match self {
            Sound::Windup => include_bytes!("../sounds/windup.wav"),
            Sound::Ding => include_bytes!("../sounds/ding.wav"),
            Sound::Tick => include_bytes!("../sounds/tick.wav"),
        }
    }

    pub fn from_name(name: &str) -> Option<Sound> {
        match name {
            "windup" => Some(Sound::Windup),
            "ding" => Some(Sound::Ding),
            "ticking" => Some(Sound::Tick),
            _ => None,
        }
    }
}

enum Cmd {
    Play(Sound, f32),
    /// Some(volume): tick (or change the tick volume). None: stop ticking.
    Ticking(Option<f32>),
}

pub struct Audio {
    tx: Mutex<Sender<Cmd>>,
}

impl Audio {
    pub fn start() -> Audio {
        let (tx, rx) = channel::<Cmd>();
        std::thread::spawn(move || {
            let Ok((_stream, handle)) = OutputStream::try_default() else {
                for _ in rx {} // no device: swallow everything
                return;
            };
            let mut tick: Option<Sink> = None;
            let decode = |s: Sound| Decoder::new(Cursor::new(s.bytes())).ok();
            loop {
                match rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(Cmd::Play(sound, vol)) => {
                        if let (Ok(sink), Some(src)) = (Sink::try_new(&handle), decode(sound)) {
                            sink.set_volume(vol);
                            sink.append(src);
                            sink.detach(); // keeps playing after we let go of it
                        }
                    }
                    Ok(Cmd::Ticking(Some(vol))) => {
                        if tick.is_none() {
                            tick = Sink::try_new(&handle).ok();
                        }
                        if let Some(s) = &tick {
                            s.set_volume(vol);
                        }
                    }
                    Ok(Cmd::Ticking(None)) => {
                        if let Some(s) = tick.take() {
                            s.stop();
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
                // Keep one tick queued: the file is one second of tick, played end to end.
                if let Some(s) = &tick {
                    if s.empty() {
                        if let Some(src) = decode(Sound::Tick) {
                            s.append(src);
                        }
                    }
                }
            }
        });
        Audio { tx: Mutex::new(tx) }
    }

    /// Plays once at `volume` (0 to 1). Zero is silence.
    pub fn play(&self, sound: Sound, volume: f64) {
        if volume > 0.0 {
            let _ = self.tx.lock().unwrap().send(Cmd::Play(sound, volume as f32));
        }
    }

    /// Starts, retunes or stops the ticking loop. A volume of zero stops it.
    pub fn ticking(&self, volume: Option<f64>) {
        let cmd = match volume {
            Some(v) if v > 0.0 => Cmd::Ticking(Some(v as f32)),
            _ => Cmd::Ticking(None),
        };
        let _ = self.tx.lock().unwrap().send(cmd);
    }
}
